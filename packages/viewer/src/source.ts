import type {
  DocumentSource,
  ViewerFetch,
  ViewerProgress,
} from "./contracts.js";
import { abortError, normalizeError, ViewerError } from "./errors.js";

export interface LoadedSource {
  readonly bytes: Uint8Array;
  readonly fileName?: string;
  readonly contentType?: string;
}

export interface SourceLoadOptions {
  readonly fetch: ViewerFetch;
  readonly signal: AbortSignal;
  readonly maxBytes: number;
  readonly reportProgress: (progress: ViewerProgress) => void;
}

export async function loadDocumentSource(
  source: DocumentSource,
  options: SourceLoadOptions,
): Promise<LoadedSource> {
  throwIfAborted(options.signal);
  if (source instanceof Uint8Array) return binary(source, options.maxBytes);
  if (source instanceof ArrayBuffer)
    return binary(new Uint8Array(source), options.maxBytes);
  if (typeof Blob !== "undefined" && source instanceof Blob) {
    enforceSize(source.size, options.maxBytes);
    const bytes = new Uint8Array(await source.arrayBuffer());
    throwIfAborted(options.signal);
    return {
      bytes,
      ...(source.type ? { contentType: source.type } : {}),
      ...("name" in source && typeof source.name === "string"
        ? { fileName: source.name }
        : {}),
    };
  }
  if (typeof source !== "string" && !(source instanceof URL))
    throw new ViewerError("invalid-file", "Unsupported document source object");

  const url =
    source instanceof URL
      ? source
      : new URL(source, globalThis.location?.href ?? "http://localhost/");
  let response: Response;
  try {
    response = await options.fetch(url, { signal: options.signal });
  } catch (error) {
    if (options.signal.aborted) throw abortError();
    throw new ViewerError("network-error", `Failed to fetch ${url.href}`, {
      cause: error,
    });
  }
  if (!response.ok)
    throw new ViewerError(
      "network-error",
      `Failed to fetch document: HTTP ${response.status}`,
      {
        details: { status: response.status, url: url.href },
      },
    );

  const declaredLength = Number(response.headers.get("content-length"));
  if (Number.isFinite(declaredLength) && declaredLength >= 0)
    enforceSize(declaredLength, options.maxBytes);
  const bytes = await readLimitedBody(response, options);
  const fileName = url.pathname.split("/").at(-1);
  const contentType = response.headers.get("content-type");
  return {
    bytes,
    ...(fileName ? { fileName } : {}),
    ...(contentType ? { contentType } : {}),
  };
}

export async function sourceToBytes(
  source: DocumentSource,
  signal?: AbortSignal,
): Promise<Uint8Array> {
  const controller = signal ? undefined : new AbortController();
  const loaded = await loadDocumentSource(source, {
    fetch: globalThis.fetch.bind(globalThis),
    signal: signal ?? controller!.signal,
    maxBytes: Number.MAX_SAFE_INTEGER,
    reportProgress: () => {},
  });
  return loaded.bytes;
}

async function readLimitedBody(
  response: Response,
  options: SourceLoadOptions,
): Promise<Uint8Array> {
  if (!response.body) {
    const bytes = new Uint8Array(await response.arrayBuffer());
    enforceSize(bytes.byteLength, options.maxBytes);
    return bytes;
  }
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let loaded = 0;
  const total = Number(response.headers.get("content-length")) || undefined;
  try {
    while (true) {
      const result = await reader.read();
      if (result.done) break;
      loaded += result.value.byteLength;
      enforceSize(loaded, options.maxBytes);
      chunks.push(result.value);
      options.reportProgress({
        phase: "loading",
        loaded,
        ...(total ? { total, ratio: loaded / total } : {}),
      });
    }
  } catch (error) {
    if (options.signal.aborted) throw abortError();
    throw normalizeError(error, "network-error");
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(loaded);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

function binary(bytes: Uint8Array, maxBytes: number): LoadedSource {
  enforceSize(bytes.byteLength, maxBytes);
  return { bytes };
}

function enforceSize(actual: number, limit: number): void {
  if (actual > limit)
    throw new ViewerError("resource-limit", "Input exceeds maxInputBytes", {
      details: { actual, limit },
    });
}

function throwIfAborted(signal: AbortSignal): void {
  if (signal.aborted) throw abortError();
}
