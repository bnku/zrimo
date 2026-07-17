import type {
  FontPolicy,
  FontRequest,
  FontResolution,
  FontScript,
  FontSource,
  RegisteredFont,
  TextRun,
  ViewerFetch,
  ViewerWarning,
} from "./contracts.js";
import { fallbackFontPacks, type FallbackFontPack } from "./font-manifest.js";

const FALLBACK_FAMILY = "Zrimo Noto";

export interface FontEnvironment {
  readonly FontFace?: typeof FontFace;
  readonly fonts?: FontFaceSet;
}

export interface FontManagerOptions {
  readonly policy?: FontPolicy;
  readonly registered?: readonly RegisteredFont[];
  readonly assetBaseUrl?: URL;
  readonly fetch: ViewerFetch;
  readonly environment?: FontEnvironment;
}

export class FontManager {
  readonly #policy: Required<Pick<FontPolicy, "mode">> & FontPolicy;
  readonly #registered: readonly RegisteredFont[];
  readonly #assetBaseUrl: URL | undefined;
  readonly #fetch: ViewerFetch;
  readonly #environment: FontEnvironment;
  readonly #loads = new Map<string, Promise<boolean>>();
  readonly #faces = new Set<FontFace>();
  readonly #warned = new Set<string>();
  #destroyed = false;

  constructor(options: FontManagerOptions) {
    this.#policy = { mode: options.policy?.mode ?? "auto", ...options.policy };
    this.#registered = options.registered ?? [];
    this.#assetBaseUrl = options.assetBaseUrl;
    this.#fetch = options.fetch;
    this.#environment = options.environment ?? {
      FontFace: globalThis.FontFace,
      fonts: globalThis.document?.fonts,
    };
  }

  async ensureRuns(
    runs: readonly TextRun[],
    reportWarning: (warning: ViewerWarning) => void,
    signal?: AbortSignal,
  ): Promise<void> {
    // PDF.js owns embedded/subset/standard-font loading for both its canvas and
    // text layer. Resolving those generated families through the viewer's Noto
    // policy would race the backend and can substitute the wrong metrics.
    const requests = fontRequestsForRuns(
      runs.filter((run) => run.textLayer !== "pdf"),
    );
    await Promise.all(
      requests.map((request) =>
        this.#ensureRequest(request, reportWarning, signal),
      ),
    );
  }

  async destroy(): Promise<void> {
    if (this.#destroyed) return;
    this.#destroyed = true;
    await Promise.allSettled(this.#loads.values());
    const fonts = this.#environment.fonts;
    if (fonts) for (const face of this.#faces) fonts.delete(face);
    this.#faces.clear();
    this.#loads.clear();
  }

  async #ensureRequest(
    request: FontRequest,
    reportWarning: (warning: ViewerWarning) => void,
    signal?: AbortSignal,
  ): Promise<void> {
    if (this.#destroyed || signal?.aborted) return;
    const matching = this.#registered.filter(
      (font) =>
        (!font.scripts || font.scripts.includes(request.script)) &&
        (!request.family || font.family === request.family),
    );
    if (matching.length > 0) {
      const loaded = await Promise.all(
        matching.map((font) =>
          this.#loadResolution(
            `registered:${font.family}:${request.script}:${font.weight ?? 400}:${font.style ?? "normal"}`,
            font,
            request,
            reportWarning,
            signal,
          ),
        ),
      );
      if (loaded.some(Boolean)) return;
    }

    if (this.#hasSystemFont(request)) return;
    if (this.#policy.mode === "offline") {
      this.#warnUnavailable(
        request,
        reportWarning,
        "offline policy has no matching registered or system font",
      );
      return;
    }
    if (this.#policy.mode === "custom") {
      const resolution = await this.#policy.resolver?.(
        request,
        signal ?? new AbortController().signal,
      );
      if (
        resolution &&
        (await this.#loadResolution(
          `custom:${request.family ?? "fallback"}:${request.script}:${request.weight}:${request.style}`,
          resolution,
          request,
          reportWarning,
          signal,
        ))
      )
        return;
      this.#warnUnavailable(
        request,
        reportWarning,
        "custom resolver returned no usable font",
      );
      return;
    }

    const packs = fallbackFontPacks.filter((pack) =>
      pack.scripts.includes(request.script),
    );
    const loaded = await Promise.all(
      packs.map((pack) => this.#loadPack(pack, request, reportWarning, signal)),
    );
    if (!loaded.some(Boolean))
      this.#warnUnavailable(
        request,
        reportWarning,
        "packaged fallback could not be loaded",
      );
  }

  #hasSystemFont(request: FontRequest): boolean {
    const fonts = this.#environment.fonts;
    if (!fonts) return false;
    if (
      !request.family &&
      request.script !== "latin" &&
      request.script !== "cyrillic"
    )
      return false;
    const family = request.family ? quoteFamily(request.family) : "system-ui";
    return fonts.check(
      `${request.style} ${request.weight} 16px ${family}`,
      String.fromCodePoint(...request.codepoints.slice(0, 32)),
    );
  }

  #loadPack(
    pack: FallbackFontPack,
    request: FontRequest,
    reportWarning: (warning: ViewerWarning) => void,
    signal?: AbortSignal,
  ): Promise<boolean> {
    const base = this.#assetBaseUrl ?? new URL("./", import.meta.url);
    return this.#loadResolution(
      `pack:${pack.id}`,
      {
        family: pack.family,
        source: new URL(`fonts/${pack.file}`, base),
        weight: 400,
        style: "normal",
      },
      request,
      reportWarning,
      signal,
      pack.unicodeRange,
    );
  }

  #loadResolution(
    key: string,
    resolution: FontResolution | RegisteredFont,
    request: FontRequest,
    reportWarning: (warning: ViewerWarning) => void,
    signal?: AbortSignal,
    unicodeRange?: string,
  ): Promise<boolean> {
    const cached = this.#loads.get(key);
    if (cached) return cached;
    const loading = this.#createFace(
      resolution.family ?? FALLBACK_FAMILY,
      resolution.source,
      resolution.weight ?? request.weight,
      resolution.style ?? request.style,
      unicodeRange,
      signal,
    ).catch((error: unknown) => {
      this.#warnOnce(`load:${key}`, reportWarning, {
        code: "font-unavailable",
        message: `Font ${resolution.family ?? FALLBACK_FAMILY} could not be loaded`,
        details: {
          script: request.script,
          reason: error instanceof Error ? error.message : String(error),
        },
      });
      return false;
    });
    this.#loads.set(key, loading);
    return loading;
  }

  async #createFace(
    family: string,
    source: FontSource,
    weight: number,
    style: string,
    unicodeRange: string | undefined,
    signal?: AbortSignal,
  ): Promise<boolean> {
    if (this.#destroyed || signal?.aborted) return false;
    const FontFaceConstructor = this.#environment.FontFace;
    const fonts = this.#environment.fonts;
    if (!FontFaceConstructor || !fonts) return false;
    const bytes = await fontBytes(source, this.#fetch, signal);
    if (signal?.aborted) return false;
    const face = new FontFaceConstructor(family, bytes, {
      weight: String(weight),
      style,
      ...(unicodeRange ? { unicodeRange } : {}),
    });
    this.#faces.add(face);
    fonts.add(face);
    try {
      await face.load();
      return true;
    } catch (error) {
      fonts.delete(face);
      this.#faces.delete(face);
      throw error;
    }
  }

  #warnUnavailable(
    request: FontRequest,
    reportWarning: (warning: ViewerWarning) => void,
    reason: string,
  ): void {
    this.#warnOnce(
      `missing:${request.script}:${request.family ?? ""}`,
      reportWarning,
      {
        code: "font-unavailable",
        message: `No font was available for ${request.script} content`,
        details: {
          script: request.script,
          requestedFamily: request.family,
          substitution: FALLBACK_FAMILY,
          reason,
        },
      },
    );
  }

  #warnOnce(
    key: string,
    reportWarning: (warning: ViewerWarning) => void,
    warning: ViewerWarning,
  ): void {
    if (this.#warned.has(key)) return;
    this.#warned.add(key);
    reportWarning(warning);
  }
}

export function fontRequestsForRuns(
  runs: readonly TextRun[],
): readonly FontRequest[] {
  const requests = new Map<string, Set<number>>();
  const details = new Map<
    string,
    {
      family?: string;
      weight: number;
      style: "normal" | "italic" | "oblique";
      script: FontScript;
    }
  >();
  for (const run of runs) {
    for (const character of run.text) {
      const codepoint = character.codePointAt(0)!;
      const script = scriptForCodepoint(codepoint);
      if (script === "unknown") continue;
      const weight = run.fontWeight ?? 400;
      const style = run.fontStyle ?? "normal";
      const key = `${run.fontFamily ?? ""}:${weight}:${style}:${script}`;
      const points = requests.get(key) ?? new Set<number>();
      points.add(codepoint);
      requests.set(key, points);
      details.set(key, {
        ...(run.fontFamily ? { family: run.fontFamily } : {}),
        weight,
        style,
        script,
      });
    }
  }
  return [...requests].map(([key, codepoints]) =>
    Object.freeze({
      ...details.get(key)!,
      codepoints: Object.freeze([...codepoints]),
    }),
  );
}

export function scriptForCodepoint(codepoint: number): FontScript {
  if (inRanges(codepoint, [0x3040, 0x30ff, 0x31f0, 0x31ff])) return "japanese";
  if (inRanges(codepoint, [0x1100, 0x11ff, 0x3130, 0x318f, 0xac00, 0xd7af]))
    return "korean";
  if (
    inRanges(
      codepoint,
      [0x2e80, 0x2fff, 0x3400, 0x4dbf, 0x4e00, 0x9fff, 0xf900, 0xfaff],
    )
  )
    return "cjk";
  if (
    inRanges(
      codepoint,
      [
        0x0600, 0x06ff, 0x0750, 0x077f, 0x08a0, 0x08ff, 0xfb50, 0xfdff, 0xfe70,
        0xfeff,
      ],
    )
  )
    return "arabic";
  if (inRanges(codepoint, [0x0900, 0x097f, 0xa8e0, 0xa8ff]))
    return "devanagari";
  if (inRanges(codepoint, [0x0980, 0x09ff])) return "bengali";
  if (inRanges(codepoint, [0x0a00, 0x0a7f])) return "gurmukhi";
  if (inRanges(codepoint, [0x0a80, 0x0aff])) return "gujarati";
  if (inRanges(codepoint, [0x0b00, 0x0b7f])) return "odia";
  if (inRanges(codepoint, [0x0b80, 0x0bff])) return "tamil";
  if (inRanges(codepoint, [0x0c00, 0x0c7f])) return "telugu";
  if (inRanges(codepoint, [0x0c80, 0x0cff])) return "kannada";
  if (inRanges(codepoint, [0x0d00, 0x0d7f])) return "malayalam";
  if (inRanges(codepoint, [0x0400, 0x052f])) return "cyrillic";
  if (
    inRanges(codepoint, [0x0000, 0x024f, 0x1e00, 0x1eff]) &&
    /\p{Script=Latin}/u.test(String.fromCodePoint(codepoint))
  )
    return "latin";
  return "unknown";
}

async function fontBytes(
  source: FontSource,
  fetch: ViewerFetch,
  signal?: AbortSignal,
): Promise<ArrayBuffer> {
  if (source instanceof ArrayBuffer) return source.slice(0);
  if (source instanceof Uint8Array)
    return source.buffer.slice(
      source.byteOffset,
      source.byteOffset + source.byteLength,
    ) as ArrayBuffer;
  const response = await fetch(source, signal ? { signal } : undefined);
  if (!response.ok)
    throw new Error(`Font request failed with HTTP ${response.status}`);
  return response.arrayBuffer();
}

function inRanges(codepoint: number, ranges: readonly number[]): boolean {
  for (let index = 0; index < ranges.length; index += 2)
    if (codepoint >= ranges[index]! && codepoint <= ranges[index + 1]!)
      return true;
  return false;
}

function quoteFamily(family: string): string {
  return `"${family.replaceAll('"', '\\"')}"`;
}
