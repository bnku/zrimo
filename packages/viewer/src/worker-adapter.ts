import type {
  AdapterOpenContext,
  DocumentAdapter,
  DocumentFormat,
  DocumentInfo,
  RenderViewport,
  TextRun,
} from "./contracts.js";
import { ViewerError } from "./errors.js";
import { WorkerRpcClient, type WorkerLike } from "./worker-client.js";
import type {
  WorkerOpenPayload,
  WorkerRenderPayload,
} from "./worker-protocol.js";

interface WorkerHandle {
  readonly rpc: WorkerRpcClient;
  readonly timeoutMs: number;
}

export interface WorkerDocumentAdapterOptions {
  readonly id: string;
  readonly formats: readonly DocumentFormat[];
  readonly createWorker: () => WorkerLike;
}

export class WorkerDocumentAdapter implements DocumentAdapter<WorkerHandle> {
  readonly id: string;
  readonly formats: readonly DocumentFormat[];
  readonly #createWorker: () => WorkerLike;
  readonly #handles = new Set<WorkerHandle>();

  constructor(options: WorkerDocumentAdapterOptions) {
    this.id = options.id;
    this.formats = options.formats;
    this.#createWorker = options.createWorker;
  }

  async open(
    data: Uint8Array,
    context: AdapterOpenContext,
  ): Promise<WorkerHandle> {
    const rpc = new WorkerRpcClient(this.#createWorker());
    const handle = { rpc, timeoutMs: context.limits.maxOperationMs };
    const transferable = data.slice().buffer;
    const payload: WorkerOpenPayload = {
      data: transferable,
      format: context.format,
      limits: context.limits,
      ...(context.fileName ? { fileName: context.fileName } : {}),
      ...(context.contentType ? { contentType: context.contentType } : {}),
    };
    try {
      await rpc.request("init", undefined, {
        signal: context.signal,
        timeoutMs: context.limits.maxOperationMs,
      });
      await rpc.request("open", payload, {
        signal: context.signal,
        transfer: [transferable],
        onProgress: context.reportProgress,
        onWarning: context.reportWarning,
        timeoutMs: context.limits.maxOperationMs,
      });
      this.#handles.add(handle);
      return handle;
    } catch (error) {
      rpc.destroy();
      throw error;
    }
  }

  getInfo(handle: WorkerHandle): Promise<DocumentInfo> {
    return handle.rpc.request<DocumentInfo>("get-info", undefined, {
      timeoutMs: handle.timeoutMs,
    });
  }

  async render(
    handle: WorkerHandle,
    target: HTMLCanvasElement | OffscreenCanvas,
    viewport: RenderViewport,
    signal?: AbortSignal,
  ): Promise<void> {
    const payload: WorkerRenderPayload = viewport;
    const bitmap = await handle.rpc.request<ImageBitmap>("render", payload, {
      ...(signal ? { signal } : {}),
      timeoutMs: handle.timeoutMs,
    });
    const bitmapContext = target.getContext(
      "bitmaprenderer",
    ) as ImageBitmapRenderingContext | null;
    if (bitmapContext) {
      bitmapContext.transferFromImageBitmap(bitmap);
      return;
    }
    const context = target.getContext("2d") as
      CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D | null;
    if (!context) {
      bitmap.close();
      throw new ViewerError(
        "render-failed",
        "Canvas has no 2D or bitmaprenderer context",
      );
    }
    context.drawImage(bitmap, 0, 0);
    bitmap.close();
  }

  getTextMap(
    handle: WorkerHandle,
    pageIndex: number,
    signal?: AbortSignal,
  ): Promise<readonly TextRun[]> {
    return handle.rpc.request<readonly TextRun[]>(
      "get-text-map",
      { pageIndex },
      { ...(signal ? { signal } : {}), timeoutMs: handle.timeoutMs },
    );
  }

  async close(handle: WorkerHandle): Promise<void> {
    if (!this.#handles.delete(handle)) return;
    try {
      await handle.rpc.request("close", undefined, {
        timeoutMs: handle.timeoutMs,
      });
    } finally {
      handle.rpc.destroy();
    }
  }

  async destroy(): Promise<void> {
    const handles = [...this.#handles];
    this.#handles.clear();
    await Promise.all(
      handles.map(async (handle) => {
        try {
          await handle.rpc.request("destroy", undefined, {
            timeoutMs: handle.timeoutMs,
          });
        } finally {
          handle.rpc.destroy();
        }
      }),
    );
  }
}
