import { normalizeError } from "./errors.js";
import type { ViewerProgress, ViewerWarning } from "./contracts.js";
import type {
  WorkerCancel,
  WorkerInboundMessage,
  WorkerOperation,
  WorkerOutboundMessage,
  WorkerRequest,
} from "./worker-protocol.js";
import { transferablesFor } from "./worker-protocol.js";

export interface WorkerScopeLike {
  postMessage(message: WorkerOutboundMessage, transfer?: Transferable[]): void;
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<WorkerInboundMessage>) => void,
  ): void;
  removeEventListener(
    type: "message",
    listener: (event: MessageEvent<WorkerInboundMessage>) => void,
  ): void;
}

export type WorkerOperationHandler = (
  operation: WorkerOperation,
  payload: unknown,
  context: {
    signal: AbortSignal;
    reportProgress: (progress: ViewerProgress) => void;
    reportWarning: (warning: ViewerWarning) => void;
  },
) => Promise<unknown>;

export function attachWorkerEndpoint(
  scope: WorkerScopeLike,
  handler: WorkerOperationHandler,
): () => void {
  const operations = new Map<number, AbortController>();
  const onMessage = (event: MessageEvent<WorkerInboundMessage>): void => {
    const message = event.data;
    if (message.kind === "cancel") {
      cancel(operations, message);
      return;
    }
    void run(scope, handler, operations, message);
  };
  scope.addEventListener("message", onMessage);
  return () => {
    scope.removeEventListener("message", onMessage);
    for (const controller of operations.values()) controller.abort();
    operations.clear();
  };
}

async function run(
  scope: WorkerScopeLike,
  handler: WorkerOperationHandler,
  operations: Map<number, AbortController>,
  request: WorkerRequest,
): Promise<void> {
  const controller = new AbortController();
  operations.set(request.id, controller);
  try {
    const result = await handler(request.operation, request.payload, {
      signal: controller.signal,
      reportProgress: (progress) =>
        scope.postMessage({
          kind: "progress",
          id: request.id,
          progress,
        }),
      reportWarning: (warning) =>
        scope.postMessage({
          kind: "warning",
          id: request.id,
          warning,
        }),
    });
    scope.postMessage(
      {
        kind: "success",
        id: request.id,
        ...(result === undefined ? {} : { result }),
      },
      transferablesFor(result),
    );
  } catch (error) {
    scope.postMessage({
      kind: "failure",
      id: request.id,
      error: normalizeError(error).toJSON(),
    });
  } finally {
    operations.delete(request.id);
  }
}

function cancel(
  operations: Map<number, AbortController>,
  message: WorkerCancel,
): void {
  operations.get(message.id)?.abort();
}
