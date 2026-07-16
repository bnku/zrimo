import type {
  AdapterOpenContext,
  DocumentAdapter,
  DocumentInfo,
  RenderViewport,
  TextRun,
} from "../contracts.js";
import { abortError, ViewerError } from "../errors.js";
import { drawEncodedImage } from "./bitmap.js";

export interface PdfBackend {
  readonly pageCount: number;
  renderPagePng(
    pageIndex: number,
    dpi: number,
    signal?: AbortSignal,
  ): Promise<Uint8Array>;
  pageTextJson(pageIndex: number, signal?: AbortSignal): Promise<string>;
  close(): void;
}

export interface PdfAdapterOptions {
  readonly workerUrl?: string | URL;
  readonly moduleUrl?: string | URL;
  readonly open?: (
    data: Uint8Array,
    context: AdapterOpenContext,
  ) => Promise<PdfBackend>;
}

interface PdfHandle {
  readonly backend: PdfBackend;
  readonly cache: Map<string, CachedPage>;
  readonly maxPixels: number;
  cachedPixels: number;
}

interface CachedPage {
  readonly png: Uint8Array;
  readonly pixels: number;
}

interface PdfPageText {
  readonly pageWidth?: number;
  readonly page_width?: number;
  readonly pageHeight?: number;
  readonly page_height?: number;
  readonly chars?: readonly PdfCharacter[];
}

interface PdfCharacter {
  readonly char: string;
  readonly bbox: {
    readonly x: number;
    readonly y: number;
    readonly width: number;
    readonly height: number;
  };
}

export class PdfDocumentAdapter implements DocumentAdapter<PdfHandle> {
  readonly id = "pdf";
  readonly formats = ["pdf"] as const;
  readonly #options: PdfAdapterOptions;

  constructor(options: PdfAdapterOptions = {}) {
    this.#options = options;
  }

  async open(
    data: Uint8Array,
    context: AdapterOpenContext,
  ): Promise<PdfHandle> {
    if (containsAscii(data, "/Encrypt"))
      throw new ViewerError(
        "encrypted-document",
        "Password-protected PDF documents are not supported",
      );
    context.reportProgress({ phase: "parsing", loaded: 0, total: 1 });
    try {
      const backend = this.#options.open
        ? await this.#options.open(data, context)
        : await WorkerPdfBackend.open(
            data,
            this.#options.workerUrl
              ? new URL(this.#options.workerUrl, context.assetBaseUrl)
              : context.assetBaseUrl
                ? new URL("workers/pdf-worker.js", context.assetBaseUrl)
                : new URL("../pdf-worker.js", import.meta.url),
            this.#options.moduleUrl
              ? new URL(this.#options.moduleUrl, context.assetBaseUrl)
              : context.assetBaseUrl
                ? new URL("wasm/pdf/index.js", context.assetBaseUrl)
                : new URL("../assets/pdf/index.js", import.meta.url),
            context.signal,
            context.limits.maxOperationMs,
          );
      if (context.signal.aborted) {
        backend.close();
        throw abortError();
      }
      if (backend.pageCount < 1) {
        backend.close();
        throw new ViewerError("invalid-file", "PDF contains no pages");
      }
      context.reportProgress({
        phase: "parsing",
        loaded: 1,
        total: 1,
        ratio: 1,
      });
      return {
        backend,
        cache: new Map(),
        maxPixels: context.limits.maxDecodedPixels,
        cachedPixels: 0,
      };
    } catch (error) {
      if (error instanceof ViewerError) throw error;
      const message = error instanceof Error ? error.message : String(error);
      throw new ViewerError(
        /encrypt|password/i.test(message)
          ? "encrypted-document"
          : "invalid-file",
        message,
        { cause: error },
      );
    }
  }

  async getInfo(handle: PdfHandle): Promise<DocumentInfo> {
    return { format: "pdf", unit: "page", pageCount: handle.backend.pageCount };
  }

  async render(
    handle: PdfHandle,
    target: HTMLCanvasElement | OffscreenCanvas,
    viewport: RenderViewport,
    signal?: AbortSignal,
  ): Promise<void> {
    assertPage(viewport.pageIndex, handle.backend.pageCount);
    if (signal?.aborted) throw abortError();
    const dpr = Math.max(1, viewport.devicePixelRatio);
    const dpi = Math.max(
      36,
      Math.min(600, Math.round(72 * viewport.zoom * dpr)),
    );
    const key = `${viewport.pageIndex}@${dpi}`;
    let page = handle.cache.get(key);
    if (page) {
      handle.cache.delete(key);
      handle.cache.set(key, page);
    } else {
      const png = await handle.backend.renderPagePng(
        viewport.pageIndex,
        dpi,
        signal,
      );
      const dimensions = pngDimensions(png);
      const pixels = dimensions.width * dimensions.height;
      if (!Number.isSafeInteger(pixels) || pixels > handle.maxPixels)
        throw new ViewerError(
          "resource-limit",
          "Rendered PDF page exceeds pixel limit",
          {
            details: { pixels, limit: handle.maxPixels },
          },
        );
      page = { png, pixels };
      while (
        handle.cachedPixels + pixels > handle.maxPixels &&
        handle.cache.size > 0
      ) {
        const oldest = handle.cache.keys().next().value as string;
        const removed = handle.cache.get(oldest)!;
        handle.cache.delete(oldest);
        handle.cachedPixels -= removed.pixels;
      }
      handle.cache.set(key, page);
      handle.cachedPixels += pixels;
    }
    await drawEncodedImage(page.png, "image/png", target, { dpr });
    if (signal?.aborted) throw abortError();
  }

  async getTextMap(
    handle: PdfHandle,
    pageIndex: number,
    signal?: AbortSignal,
  ): Promise<readonly TextRun[]> {
    assertPage(pageIndex, handle.backend.pageCount);
    if (signal?.aborted) throw abortError();
    let parsed: PdfPageText;
    try {
      parsed = JSON.parse(
        await handle.backend.pageTextJson(pageIndex, signal),
      ) as PdfPageText;
    } catch (error) {
      throw new ViewerError("invalid-file", "Invalid PDF text-map response", {
        cause: error,
      });
    }
    return (parsed.chars ?? []).map((character) => ({
      text: character.char,
      x: character.bbox.x,
      y: character.bbox.y,
      width: character.bbox.width,
      height: character.bbox.height,
      direction: /[\u0590-\u08ff\ufb1d-\ufefc]/u.test(character.char)
        ? "rtl"
        : "ltr",
    }));
  }

  close(handle: PdfHandle): void {
    handle.cache.clear();
    handle.cachedPixels = 0;
    handle.backend.close();
  }
}

export function createPdfAdapter(
  options: PdfAdapterOptions = {},
): PdfDocumentAdapter {
  return new PdfDocumentAdapter(options);
}

class WorkerPdfBackend implements PdfBackend {
  readonly pageCount: number;
  readonly #worker: Worker;
  readonly #pending = new Map<
    number,
    { resolve(value: unknown): void; reject(error: unknown): void }
  >();
  #requestId = 0;
  #closed = false;
  readonly #timeoutMs: number;

  private constructor(worker: Worker, pageCount: number, timeoutMs: number) {
    this.#worker = worker;
    this.pageCount = pageCount;
    this.#timeoutMs = timeoutMs;
    worker.onmessage = (event: MessageEvent<PdfWorkerResponse>) => {
      const pending = this.#pending.get(event.data.id);
      if (!pending) return;
      this.#pending.delete(event.data.id);
      if (event.data.ok) pending.resolve(event.data);
      else pending.reject(new ViewerError("invalid-file", event.data.message));
    };
    worker.onerror = (event) => {
      for (const pending of this.#pending.values())
        pending.reject(
          new ViewerError("worker-crashed", "PDF worker crashed", {
            details: { message: event.message },
          }),
        );
      this.#pending.clear();
    };
  }

  static async open(
    data: Uint8Array,
    workerUrl: URL,
    moduleUrl: URL,
    signal: AbortSignal,
    timeoutMs: number,
  ): Promise<WorkerPdfBackend> {
    const worker = new Worker(workerUrl, {
      type: "module",
      name: "pdf-renderer",
    });
    const copy = data.slice().buffer;
    const response = await oneShotRequest(
      worker,
      { id: 0, type: "open", data: copy, moduleUrl: moduleUrl.href },
      [copy],
      signal,
      timeoutMs,
    );
    if (!response.ok || typeof response.pageCount !== "number") {
      worker.terminate();
      throw new ViewerError(
        "invalid-file",
        response.ok ? "PDF worker returned no page count" : response.message,
      );
    }
    return new WorkerPdfBackend(worker, response.pageCount, timeoutMs);
  }

  async renderPagePng(
    pageIndex: number,
    dpi: number,
    signal?: AbortSignal,
  ): Promise<Uint8Array> {
    const response = (await this.#request(
      { type: "render", pageIndex, dpi },
      signal,
    )) as PdfWorkerResponse;
    if (!response.ok || !response.data)
      throw new ViewerError("render-failed", "PDF worker returned no bitmap");
    return new Uint8Array(response.data);
  }

  async pageTextJson(pageIndex: number, signal?: AbortSignal): Promise<string> {
    const response = (await this.#request(
      { type: "text", pageIndex },
      signal,
    )) as PdfWorkerResponse;
    if (!response.ok || response.json === undefined)
      throw new ViewerError("invalid-file", "PDF worker returned no text map");
    return response.json;
  }

  close(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#worker.postMessage({ id: ++this.#requestId, type: "close" });
    for (const pending of this.#pending.values())
      pending.reject(abortError("PDF closed"));
    this.#pending.clear();
    this.#worker.terminate();
  }

  #request(
    payload: Omit<PdfWorkerRequest, "id">,
    signal?: AbortSignal,
  ): Promise<unknown> {
    if (this.#closed)
      return Promise.reject(
        new ViewerError("lifecycle-error", "PDF backend is closed"),
      );
    if (signal?.aborted) return Promise.reject(abortError());
    const id = ++this.#requestId;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.#pending.delete(id);
        this.close();
        reject(workerTimeout("PDF", this.#timeoutMs));
      }, this.#timeoutMs);
      const onAbort = (): void => {
        clearTimeout(timeout);
        this.#pending.delete(id);
        reject(abortError());
      };
      signal?.addEventListener("abort", onAbort, { once: true });
      this.#pending.set(id, {
        resolve: (value) => {
          clearTimeout(timeout);
          signal?.removeEventListener("abort", onAbort);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timeout);
          signal?.removeEventListener("abort", onAbort);
          reject(error);
        },
      });
      this.#worker.postMessage({ id, ...payload });
    });
  }
}

interface PdfWorkerRequest {
  readonly id: number;
  readonly type: "open" | "render" | "text" | "close";
  readonly pageIndex?: number;
  readonly dpi?: number;
}

type PdfWorkerResponse =
  | {
      readonly id: number;
      readonly ok: true;
      readonly pageCount?: number;
      readonly data?: ArrayBuffer;
      readonly json?: string;
    }
  | { readonly id: number; readonly ok: false; readonly message: string };

function oneShotRequest(
  worker: Worker,
  payload: Readonly<Record<string, unknown>>,
  transfer: Transferable[],
  signal: AbortSignal,
  timeoutMs: number,
): Promise<PdfWorkerResponse> {
  return new Promise((resolve, reject) => {
    const finish = (): void => {
      clearTimeout(timeout);
      signal.removeEventListener("abort", onAbort);
    };
    const onAbort = (): void => {
      finish();
      worker.terminate();
      reject(abortError());
    };
    const timeout = setTimeout(() => {
      finish();
      worker.terminate();
      reject(workerTimeout("PDF", timeoutMs));
    }, timeoutMs);
    signal.addEventListener("abort", onAbort, { once: true });
    worker.onerror = (event) => {
      finish();
      worker.terminate();
      reject(
        new ViewerError("worker-crashed", "PDF worker crashed", {
          details: { message: event.message },
        }),
      );
    };
    worker.onmessage = (event: MessageEvent<PdfWorkerResponse>) => {
      finish();
      resolve(event.data);
    };
    worker.postMessage(payload, transfer);
  });
}

function workerTimeout(kind: string, timeoutMs: number): ViewerError {
  return new ViewerError(
    "resource-limit",
    `${kind} worker operation exceeded maxOperationMs`,
    { details: { timeoutMs } },
  );
}

function assertPage(pageIndex: number, pageCount: number): void {
  if (!Number.isInteger(pageIndex) || pageIndex < 0 || pageIndex >= pageCount)
    throw new ViewerError("render-failed", "PDF page index is out of range", {
      details: { pageIndex, pageCount },
    });
}

function pngDimensions(data: Uint8Array): { width: number; height: number } {
  if (
    data.length < 24 ||
    data[0] !== 0x89 ||
    String.fromCharCode(...data.subarray(1, 4)) !== "PNG"
  )
    throw new ViewerError(
      "render-failed",
      "PDF backend returned invalid PNG data",
    );
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  return { width: view.getUint32(16), height: view.getUint32(20) };
}

function containsAscii(data: Uint8Array, needle: string): boolean {
  const bytes = new TextEncoder().encode(needle);
  outer: for (
    let offset = 0;
    offset <= data.length - bytes.length;
    offset += 1
  ) {
    for (let index = 0; index < bytes.length; index += 1)
      if (data[offset + index] !== bytes[index]) continue outer;
    return true;
  }
  return false;
}
