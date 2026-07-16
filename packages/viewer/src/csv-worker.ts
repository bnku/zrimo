import { parseDelimitedBytes } from "./adapters/csv-parser.js";

interface Request {
  readonly data: ArrayBuffer;
  readonly format: "csv" | "tsv";
  readonly maxCells: number;
}

const workerScope = self as unknown as DedicatedWorkerGlobalScope;
workerScope.onmessage = (event: MessageEvent<Request>) => {
  try {
    const result = parseDelimitedBytes(
      new Uint8Array(event.data.data),
      event.data.format,
      event.data.maxCells,
    );
    workerScope.postMessage({ ok: true, result });
  } catch (error) {
    workerScope.postMessage({
      ok: false,
      message: error instanceof Error ? error.message : String(error),
      code:
        typeof error === "object" && error !== null && "code" in error
          ? String(error.code)
          : "invalid-file",
    });
  }
};
