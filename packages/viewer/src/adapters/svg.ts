import type {
  AdapterOpenContext,
  DocumentAdapter,
  DocumentInfo,
  RenderViewport,
  ViewerWarning,
} from "../contracts.js";
import { abortError, ViewerError } from "../errors.js";
import { drawEncodedImage } from "./bitmap.js";

interface SvgHandle {
  readonly data: Uint8Array;
  readonly warnings: readonly ViewerWarning[];
}

export class SvgDocumentAdapter implements DocumentAdapter<SvgHandle> {
  readonly id = "svg";
  readonly formats = ["svg"] as const;

  async open(
    data: Uint8Array,
    context: AdapterOpenContext,
  ): Promise<SvgHandle> {
    if (context.signal.aborted) throw abortError();
    let source: string;
    try {
      source = new TextDecoder("utf-8", { fatal: true }).decode(data);
    } catch (error) {
      throw new ViewerError("invalid-file", "SVG must be valid UTF-8", {
        cause: error,
      });
    }
    const sanitized = sanitizeSvg(source);
    const changed = sanitized !== source;
    return {
      data: new TextEncoder().encode(sanitized),
      warnings: changed
        ? [
            {
              code: "external-resource-blocked",
              message: "Active SVG content or external resources were removed",
            },
          ]
        : [],
    };
  }

  async getInfo(handle: SvgHandle): Promise<DocumentInfo> {
    return {
      format: "svg",
      unit: "image",
      pageCount: 1,
      warnings: handle.warnings,
    };
  }

  async render(
    handle: SvgHandle,
    target: HTMLCanvasElement | OffscreenCanvas,
    viewport: RenderViewport,
    signal?: AbortSignal,
  ): Promise<void> {
    if (viewport.pageIndex !== 0)
      throw new ViewerError("render-failed", "SVG page index is out of range");
    if (signal?.aborted) throw abortError();
    await drawEncodedImage(handle.data, "image/svg+xml", target, {
      dpr: viewport.devicePixelRatio,
      scale: viewport.zoom,
      ...(viewport.width === undefined ? {} : { cssWidth: viewport.width }),
      ...(viewport.height === undefined ? {} : { cssHeight: viewport.height }),
    });
    if (signal?.aborted) throw abortError();
  }

  async getTextMap(): Promise<readonly []> {
    return [];
  }

  close(): void {}
}

export function sanitizeSvg(source: string): string {
  const withoutDoctype = source.replace(/<!DOCTYPE[\s\S]*?>/gi, "");
  if (typeof DOMParser === "undefined")
    return sanitizeSvgFallback(withoutDoctype);
  const document = new DOMParser().parseFromString(
    withoutDoctype,
    "image/svg+xml",
  );
  if (
    document.querySelector("parsererror") ||
    document.documentElement.localName !== "svg"
  )
    throw new ViewerError("invalid-file", "Malformed SVG document");
  const blocked = new Set([
    "script",
    "style",
    "foreignobject",
    "iframe",
    "object",
    "embed",
    "audio",
    "video",
    "canvas",
  ]);
  for (const element of [...document.querySelectorAll("*")]) {
    if (blocked.has(element.localName.toLowerCase())) {
      element.remove();
      continue;
    }
    for (const attribute of [...element.attributes]) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value.trim();
      if (
        name.startsWith("on") ||
        ((name === "href" || name === "xlink:href" || name === "src") &&
          !value.startsWith("#")) ||
        (/url\s*\(/i.test(value) && !/url\s*\(\s*['"]?#/i.test(value)) ||
        /expression\s*\(|@import/i.test(value)
      )
        element.removeAttribute(attribute.name);
    }
  }
  return new XMLSerializer().serializeToString(document.documentElement);
}

export function createSvgAdapter(): SvgDocumentAdapter {
  return new SvgDocumentAdapter();
}

function sanitizeSvgFallback(source: string): string {
  let result = source;
  for (const tag of [
    "script",
    "style",
    "foreignObject",
    "iframe",
    "object",
    "embed",
    "audio",
    "video",
    "canvas",
  ]) {
    const paired = new RegExp(`<${tag}\\b[\\s\\S]*?<\\/${tag}\\s*>`, "gi");
    const single = new RegExp(`<${tag}\\b[^>]*\\/?>`, "gi");
    result = result.replace(paired, "").replace(single, "");
  }
  result = result
    .replace(/\s+on[\w:-]+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)/gi, "")
    .replace(
      /\s+(?:href|xlink:href|src)\s*=\s*(?:"(?!#)[^"]*"|'(?!#)[^']*'|(?!#)[^\s>"']+)/gi,
      "",
    )
    .replace(
      /\s+style\s*=\s*(?:"[^"]*(?:url|expression|@import)[^"]*"|'[^']*(?:url|expression|@import)[^']*')/gi,
      "",
    );
  if (!/<svg(?:\s|>)/i.test(result))
    throw new ViewerError("invalid-file", "Malformed SVG document");
  return result;
}
