import type { ViewerProgress, ViewerWarning } from "./contracts.js";
import { abortError, errorFromData, ViewerError } from "./errors.js";
import type {
  WorkerInboundMessage,
  WorkerOperation,
  WorkerOutboundMessage,
  WorkerRequest,
} from "./worker-protocol.js";
import { transferablesFor } from "./worker-protocol.js";

export interface WorkerLike {
  postMessage(message: WorkerInboundMessage, transfer?: Transferable[]): void;
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<WorkerOutboundMessage>) => void,
  ): void;
  addEventListener(
    type: "error" | "messageerror",
    listener: (event: Event) => void,
  ): void;
  removeEventListener(
    type: "message",
    listener: (event: MessageEvent<WorkerOutboundMessage>) => void,
  ): void;
  removeEventListener(
    type: "error" | "messageerror",
    listener: (event: Event) => void,
  ): void;
  terminate(): void;
}

export interface WorkerRequestOptions {
  readonly signal?: AbortSignal;
  readonly onProgress?: (progress: ViewerProgress) => void;
  readonly onWarning?: (warning: ViewerWarning) => void;
  readonly transfer?: Transferable[];
  readonly timeoutMs?: number;
}

interface PendingRequest {
  readonly resolve: (value: unknown) => void;
  readonly reject: (error: unknown) => void;
  readonly onProgress?: (progress: ViewerProgress) => void;
  readonly onWarning?: (warning: ViewerWarning) => void;
  readonly signal?: AbortSignal;
  readonly abort?: () => void;
  readonly timeout?: ReturnType<typeof setTimeout>;
}

export class WorkerRpcClient {
  readonly #worker: WorkerLike;
  readonly #pending = new Map<number, PendingRequest>();
  #nextId = 1;
  #destroyed = false;

  constructor(worker: WorkerLike) {
    this.#worker = worker;
    worker.addEventListener("message", this.#onMessage);
    worker.addEventListener("error", this.#onCrash);
    worker.addEventListener("messageerror", this.#onCrash);
  }

  request<T>(
    operation: WorkerOperation,
    payload?: unknown,
    options: WorkerRequestOptions = {},
  ): Promise<T> {
    if (this.#destroyed)
      return Promise.reject(
        new ViewerError("lifecycle-error", "Worker client is destroyed"),
      );
    if (options.signal?.aborted) return Promise.reject(abortError());
    const id = this.#nextId++;
    const message: WorkerRequest = {
      kind: "request",
      id,
      operation,
      ...(payload === undefined ? {} : { payload }),
    };
    return new Promise<T>((resolve, reject) => {
      const abort = options.signal
        ? () => {
            this.#worker.postMessage({ kind: "cancel", id });
            this.#finish(id, abortError());
          }
        : undefined;
      const pending: PendingRequest = {
        resolve: resolve as (value: unknown) => void,
        reject,
        ...(options.onProgress ? { onProgress: options.onProgress } : {}),
        ...(options.onWarning ? { onWarning: options.onWarning } : {}),
        ...(options.signal ? { signal: options.signal } : {}),
        ...(abort ? { abort } : {}),
        ...(options.timeoutMs
          ? {
              timeout: setTimeout(() => {
                const error = operationTimeout(operation, options.timeoutMs!);
                this.#worker.postMessage({ kind: "cancel", id });
                this.#terminate(error);
              }, options.timeoutMs),
            }
          : {}),
      };
      this.#pending.set(id, pending);
      options.signal?.addEventListener("abort", abort!, { once: true });
      this.#worker.postMessage(
        message,
        options.transfer ?? transferablesFor(payload),
      );
    });
  }

  destroy(): void {
    this.#terminate(new ViewerError("worker-crashed", "Worker terminated"));
  }

  #terminate(error: ViewerError): void {
    if (this.#destroyed) return;
    this.#destroyed = true;
    this.#worker.removeEventListener("message", this.#onMessage);
    this.#worker.removeEventListener("error", this.#onCrash);
    this.#worker.removeEventListener("messageerror", this.#onCrash);
    this.#worker.terminate();
    for (const id of this.#pending.keys()) this.#finish(id, error);
  }

  readonly #onMessage = (event: MessageEvent<WorkerOutboundMessage>): void => {
    const message = event.data;
    const pending = this.#pending.get(message.id);
    if (!pending) return;
    if (message.kind === "progress") pending.onProgress?.(message.progress);
    else if (message.kind === "warning") pending.onWarning?.(message.warning);
    else if (message.kind === "success")
      this.#finish(message.id, undefined, message.result);
    else this.#finish(message.id, errorFromData(message.error));
  };

  readonly #onCrash = (): void => {
    const error = new ViewerError("worker-crashed", "Document worker crashed");
    for (const id of this.#pending.keys()) this.#finish(id, error);
  };

  #finish(id: number, error?: unknown, value?: unknown): void {
    const pending = this.#pending.get(id);
    if (!pending) return;
    this.#pending.delete(id);
    if (pending.abort && pending.signal)
      pending.signal.removeEventListener("abort", pending.abort);
    if (pending.timeout) clearTimeout(pending.timeout);
    if (error) pending.reject(error);
    else pending.resolve(value);
  }
}

function operationTimeout(
  operation: WorkerOperation,
  timeoutMs: number,
): ViewerError {
  return new ViewerError(
    "resource-limit",
    `Worker operation ${operation} exceeded maxOperationMs`,
    { details: { operation, timeoutMs } },
  );
}
