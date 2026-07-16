import type {
  DocumentFormat,
  DocumentInfo,
  ResourceLimits,
  TextRun,
  ViewerErrorData,
  ViewerProgress,
  ViewerWarning,
} from "./contracts.js";

export type WorkerOperation =
  | "init"
  | "open"
  | "get-info"
  | "render"
  | "get-text-map"
  | "close"
  | "destroy";

export interface WorkerRequest {
  readonly kind: "request";
  readonly id: number;
  readonly operation: WorkerOperation;
  readonly payload?: unknown;
}

export interface WorkerCancel {
  readonly kind: "cancel";
  readonly id: number;
}

export interface WorkerSuccess {
  readonly kind: "success";
  readonly id: number;
  readonly result?: unknown;
}

export interface WorkerFailure {
  readonly kind: "failure";
  readonly id: number;
  readonly error: ViewerErrorData;
}

export interface WorkerProgressMessage {
  readonly kind: "progress";
  readonly id: number;
  readonly progress: ViewerProgress;
}

export interface WorkerWarningMessage {
  readonly kind: "warning";
  readonly id: number;
  readonly warning: ViewerWarning;
}

export type WorkerInboundMessage = WorkerRequest | WorkerCancel;
export type WorkerOutboundMessage =
  WorkerSuccess | WorkerFailure | WorkerProgressMessage | WorkerWarningMessage;

export interface WorkerOpenPayload {
  readonly data: ArrayBuffer;
  readonly format: DocumentFormat;
  readonly limits: ResourceLimits;
  readonly fileName?: string;
  readonly contentType?: string;
}

export interface WorkerRenderPayload {
  readonly pageIndex: number;
  readonly zoom: number;
  readonly devicePixelRatio: number;
}

export type WorkerOperationResult =
  DocumentInfo | readonly TextRun[] | ImageBitmap | ArrayBuffer | undefined;

export function transferablesFor(value: unknown): Transferable[] {
  if (value instanceof ArrayBuffer) return [value];
  if (typeof ImageBitmap !== "undefined" && value instanceof ImageBitmap)
    return [value];
  if (ArrayBuffer.isView(value)) return [value.buffer];
  if (value && typeof value === "object") {
    const data = (value as { data?: unknown }).data;
    if (data instanceof ArrayBuffer) return [data];
  }
  return [];
}
