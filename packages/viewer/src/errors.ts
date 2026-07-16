import type { ViewerErrorCode, ViewerErrorData } from "./contracts.js";

export class ViewerError extends Error implements ViewerErrorData {
  readonly name = "ViewerError" as const;
  readonly code: ViewerErrorCode;
  readonly details?: Readonly<Record<string, unknown>>;

  constructor(
    code: ViewerErrorCode,
    message: string,
    options: {
      cause?: unknown;
      details?: Readonly<Record<string, unknown>>;
    } = {},
  ) {
    super(
      message,
      options.cause === undefined ? undefined : { cause: options.cause },
    );
    this.code = code;
    if (options.details) this.details = options.details;
  }

  toJSON(): ViewerErrorData {
    return {
      name: this.name,
      code: this.code,
      message: this.message,
      ...(this.details ? { details: this.details } : {}),
    };
  }
}

export function abortError(message = "Operation aborted"): ViewerError {
  return new ViewerError("aborted", message);
}

export function normalizeError(
  error: unknown,
  fallback: ViewerErrorCode = "internal",
): ViewerError {
  if (error instanceof ViewerError) return error;
  if (isAbort(error)) return abortError();
  return new ViewerError(
    fallback,
    error instanceof Error ? error.message : String(error),
    {
      cause: error,
    },
  );
}

export function errorFromData(data: ViewerErrorData): ViewerError {
  return new ViewerError(
    data.code,
    data.message,
    data.details ? { details: data.details } : {},
  );
}

function isAbort(error: unknown): boolean {
  return (
    (typeof DOMException !== "undefined" &&
      error instanceof DOMException &&
      error.name === "AbortError") ||
    (error instanceof Error && error.name === "AbortError")
  );
}
