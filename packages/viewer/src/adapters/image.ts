import type {
  AdapterOpenContext,
  DocumentAdapter,
  DocumentFormat,
  DocumentInfo,
  RenderViewport,
  ViewerWarning,
} from "../contracts.js";
import { abortError, ViewerError } from "../errors.js";
import { drawEncodedImage } from "./bitmap.js";

type NativeImageFormat = "png" | "jpeg" | "gif" | "webp" | "bmp";

interface NativeHandle {
  readonly kind: "native";
  readonly format: NativeImageFormat;
  readonly data: Uint8Array;
  readonly warnings: readonly ViewerWarning[];
}

interface TiffHandle {
  readonly kind: "tiff";
  readonly format: "tiff";
  readonly backend: TiffBackend;
  readonly warnings: readonly ViewerWarning[];
}

type ImageHandle = NativeHandle | TiffHandle;

export interface TiffBackend {
  readonly pages: readonly {
    readonly width: number;
    readonly height: number;
  }[];
  renderPagePng(pageIndex: number, signal?: AbortSignal): Promise<Uint8Array>;
  close(): void;
}

export interface ImageAdapterOptions {
  readonly workerUrl?: string | URL;
  readonly moduleUrl?: string | URL;
  readonly openTiff?: (
    data: Uint8Array,
    context: AdapterOpenContext,
  ) => Promise<TiffBackend>;
}

export class ImageDocumentAdapter implements DocumentAdapter<ImageHandle> {
  readonly id = "image";
  readonly formats = ["png", "jpeg", "gif", "webp", "bmp", "tiff"] as const;
  readonly #options: ImageAdapterOptions;

  constructor(options: ImageAdapterOptions = {}) {
    this.#options = options;
  }

  async open(
    data: Uint8Array,
    context: AdapterOpenContext,
  ): Promise<ImageHandle> {
    if (context.format === "tiff") {
      context.reportProgress({ phase: "parsing", loaded: 0, total: 1 });
      const backend = this.#options.openTiff
        ? await this.#options.openTiff(data, context)
        : await WorkerTiffBackend.open(
            data,
            this.#options.workerUrl
              ? new URL(this.#options.workerUrl, context.assetBaseUrl)
              : context.assetBaseUrl
                ? new URL("workers/image-worker.js", context.assetBaseUrl)
                : new URL("../image-worker.js", import.meta.url),
            this.#options.moduleUrl
              ? new URL(this.#options.moduleUrl, context.assetBaseUrl)
              : context.assetBaseUrl
                ? new URL("wasm/image/index.js", context.assetBaseUrl)
                : new URL("../assets/image/index.js", import.meta.url),
            context.limits.maxDecodedPixels,
            context.signal,
            context.limits.maxOperationMs,
          );
      if (backend.pages.length === 0) {
        backend.close();
        throw new ViewerError("invalid-file", "TIFF contains no pages");
      }
      context.reportProgress({
        phase: "parsing",
        loaded: 1,
        total: 1,
        ratio: 1,
      });
      return { kind: "tiff", format: "tiff", backend, warnings: [] };
    }
    const format = context.format as NativeImageFormat;
    const animated = isAnimated(data, format);
    return {
      kind: "native",
      format,
      data,
      warnings: animated
        ? [
            {
              code: "unsupported-feature",
              message:
                "Animated image frames are preserved in the source; headless canvas rendering captures one browser-decoded frame",
              details: { feature: "animation", format },
            },
          ]
        : [],
    };
  }

  async getInfo(handle: ImageHandle): Promise<DocumentInfo> {
    return {
      format: handle.format,
      unit: "image",
      pageCount: handle.kind === "tiff" ? handle.backend.pages.length : 1,
      warnings: handle.warnings,
    };
  }

  async render(
    handle: ImageHandle,
    target: HTMLCanvasElement | OffscreenCanvas,
    viewport: RenderViewport,
    signal?: AbortSignal,
  ): Promise<void> {
    if (signal?.aborted) throw abortError();
    const pageCount = handle.kind === "tiff" ? handle.backend.pages.length : 1;
    if (
      !Number.isInteger(viewport.pageIndex) ||
      viewport.pageIndex < 0 ||
      viewport.pageIndex >= pageCount
    )
      throw new ViewerError(
        "render-failed",
        "Image page index is out of range",
      );
    const data =
      handle.kind === "tiff"
        ? await handle.backend.renderPagePng(viewport.pageIndex, signal)
        : handle.data;
    await drawEncodedImage(
      data,
      handle.kind === "tiff" ? "image/png" : mimeType(handle.format),
      target,
      {
        dpr: viewport.devicePixelRatio,
        scale: viewport.zoom,
        ...(viewport.width === undefined ? {} : { cssWidth: viewport.width }),
        ...(viewport.height === undefined
          ? {}
          : { cssHeight: viewport.height }),
      },
    );
    if (signal?.aborted) throw abortError();
  }

  async getTextMap(): Promise<readonly []> {
    return [];
  }

  close(handle: ImageHandle): void {
    if (handle.kind === "tiff") handle.backend.close();
  }
}

export function createImageAdapter(
  options: ImageAdapterOptions = {},
): ImageDocumentAdapter {
  return new ImageDocumentAdapter(options);
}

class WorkerTiffBackend implements TiffBackend {
  readonly pages: readonly {
    readonly width: number;
    readonly height: number;
  }[];
  readonly #worker: Worker;
  #id = 0;
  #closed = false;
  readonly #timeoutMs: number;

  private constructor(
    worker: Worker,
    pages: readonly { readonly width: number; readonly height: number }[],
    timeoutMs: number,
  ) {
    this.#worker = worker;
    this.pages = pages;
    this.#timeoutMs = timeoutMs;
  }

  static async open(
    data: Uint8Array,
    workerUrl: URL,
    moduleUrl: URL,
    maxPixels: number,
    signal: AbortSignal,
    timeoutMs: number,
  ): Promise<WorkerTiffBackend> {
    const worker = new Worker(workerUrl, {
      type: "module",
      name: "tiff-decoder",
    });
    const copy = data.slice().buffer;
    const response = await requestWorker(
      worker,
      {
        id: 0,
        type: "open",
        data: copy,
        moduleUrl: moduleUrl.href,
        maxPixels,
      },
      [copy],
      signal,
      timeoutMs,
    );
    if (!response.ok || !response.pages) {
      worker.terminate();
      const message = response.ok
        ? "TIFF worker returned no pages"
        : response.message;
      throw new ViewerError(
        /pixel|limit/i.test(message) ? "resource-limit" : "invalid-file",
        message,
      );
    }
    return new WorkerTiffBackend(worker, response.pages, timeoutMs);
  }

  async renderPagePng(
    pageIndex: number,
    signal?: AbortSignal,
  ): Promise<Uint8Array> {
    if (this.#closed)
      throw new ViewerError("lifecycle-error", "TIFF backend is closed");
    const response = await requestWorker(
      this.#worker,
      { id: ++this.#id, type: "render", pageIndex },
      [],
      signal,
      this.#timeoutMs,
    );
    if (!response.ok || !response.data)
      throw new ViewerError(
        "render-failed",
        response.ok ? "TIFF worker returned no bitmap" : response.message,
      );
    return new Uint8Array(response.data);
  }

  close(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#worker.postMessage({ id: ++this.#id, type: "close" });
    this.#worker.terminate();
  }
}

type ImageWorkerResponse =
  | {
      readonly id: number;
      readonly ok: true;
      readonly pages?: readonly {
        readonly width: number;
        readonly height: number;
      }[];
      readonly data?: ArrayBuffer;
    }
  | { readonly id: number; readonly ok: false; readonly message: string };

function requestWorker(
  worker: Worker,
  payload: Readonly<Record<string, unknown>>,
  transfer: Transferable[],
  signal?: AbortSignal,
  timeoutMs = 30_000,
): Promise<ImageWorkerResponse> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(abortError());
      return;
    }
    const expectedId = payload.id;
    const finish = (): void => {
      clearTimeout(timeout);
      signal?.removeEventListener("abort", onAbort);
      worker.removeEventListener("message", onMessage);
      worker.removeEventListener("error", onError);
    };
    const timeout = setTimeout(() => {
      finish();
      worker.terminate();
      reject(
        new ViewerError(
          "resource-limit",
          "TIFF worker operation exceeded maxOperationMs",
          { details: { timeoutMs } },
        ),
      );
    }, timeoutMs);
    const onAbort = (): void => {
      finish();
      reject(abortError());
    };
    const onMessage = (event: MessageEvent<ImageWorkerResponse>): void => {
      if (event.data.id !== expectedId) return;
      finish();
      resolve(event.data);
    };
    const onError = (event: ErrorEvent): void => {
      finish();
      reject(
        new ViewerError("worker-crashed", "TIFF worker crashed", {
          details: { message: event.message },
        }),
      );
    };
    signal?.addEventListener("abort", onAbort, { once: true });
    worker.addEventListener("message", onMessage);
    worker.addEventListener("error", onError);
    worker.postMessage(payload, transfer);
  });
}

function mimeType(format: NativeImageFormat): string {
  return format === "jpeg" ? "image/jpeg" : `image/${format}`;
}

function isAnimated(data: Uint8Array, format: NativeImageFormat): boolean {
  if (format === "gif") {
    let frames = 0;
    for (const byte of data) if (byte === 0x2c && ++frames > 1) return true;
  }
  if (format === "webp")
    return new TextDecoder("latin1")
      .decode(data.subarray(0, 64))
      .includes("ANIM");
  return false;
}
