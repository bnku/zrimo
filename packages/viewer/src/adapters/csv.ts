import type {
  AdapterOpenContext,
  DocumentAdapter,
  DocumentInfo,
  RenderViewport,
  SpreadsheetSheetInfo,
  TextRun,
  ViewerWarning,
} from "../contracts.js";
import { abortError, ViewerError } from "../errors.js";
import type { ParsedDelimitedText } from "./csv-parser.js";

const ROW_HEIGHT = 24;
const COLUMN_WIDTH = 120;
const ROW_HEADER_WIDTH = 48;
const COLUMN_HEADER_HEIGHT = 28;

interface CsvHandle {
  readonly format: "csv" | "tsv";
  readonly parsed: ParsedDelimitedText;
  readonly info: SpreadsheetSheetInfo;
  readonly warnings: readonly ViewerWarning[];
}

export interface CsvAdapterOptions {
  readonly workerUrl?: string | URL;
  readonly parse?: (
    data: Uint8Array,
    format: "csv" | "tsv",
    maxCells: number,
    signal: AbortSignal,
  ) => Promise<ParsedDelimitedText>;
  readonly maxCells?: number;
}

export class CsvDocumentAdapter implements DocumentAdapter<CsvHandle> {
  readonly id = "delimited-data";
  readonly formats = ["csv", "tsv"] as const;
  readonly #options: CsvAdapterOptions;

  constructor(options: CsvAdapterOptions = {}) {
    this.#options = options;
  }

  async open(
    data: Uint8Array,
    context: AdapterOpenContext,
  ): Promise<CsvHandle> {
    const format = context.format === "tsv" ? "tsv" : "csv";
    context.reportProgress({ phase: "parsing", loaded: 0, total: 1 });
    const maxCells = this.#options.maxCells ?? context.limits.maxCsvCells;
    const parsed = this.#options.parse
      ? await this.#options.parse(data, format, maxCells, context.signal)
      : await parseInWorker(
          data,
          format,
          maxCells,
          this.#options.workerUrl
            ? new URL(this.#options.workerUrl, context.assetBaseUrl)
            : context.assetBaseUrl
              ? new URL("workers/csv-worker.js", context.assetBaseUrl)
              : new URL("../csv-worker.js", import.meta.url),
          context.signal,
        );
    if (context.signal.aborted) throw abortError();
    const maxColumn = parsed.rows.reduce(
      (maximum, row) => Math.max(maximum, row.length),
      0,
    );
    const name = context.fileName?.replace(/\.[^.]+$/, "") || "Data";
    const warnings: ViewerWarning[] =
      parsed.encoding === "windows-1252"
        ? [
            {
              code: "fidelity-degraded",
              message:
                "Input was not valid UTF-8 and was decoded as Windows-1252",
              details: { encoding: parsed.encoding },
            },
          ]
        : [];
    context.reportProgress({ phase: "parsing", loaded: 1, total: 1, ratio: 1 });
    return {
      format,
      parsed,
      warnings,
      info: {
        name,
        frozenRows: 0,
        frozenColumns: 0,
        mergedRanges: [],
        maxRow: parsed.rows.length,
        maxColumn,
      },
    };
  }

  async getInfo(handle: CsvHandle): Promise<DocumentInfo> {
    return {
      format: handle.format,
      unit: "sheet",
      pageCount: 1,
      sheetNames: [handle.info.name],
      sheets: [handle.info],
      warnings: handle.warnings,
    };
  }

  async render(
    handle: CsvHandle,
    target: HTMLCanvasElement | OffscreenCanvas,
    viewport: RenderViewport,
    signal?: AbortSignal,
  ): Promise<void> {
    if (signal?.aborted) throw abortError();
    if (viewport.pageIndex !== 0)
      throw new ViewerError("render-failed", "CSV sheet index is out of range");
    const range = viewport.sheetRange ?? {
      row: 1,
      column: 1,
      rowCount: 50,
      columnCount: 20,
    };
    const zoom = Math.max(0.1, viewport.zoom);
    const width =
      viewport.width ??
      ROW_HEADER_WIDTH + range.columnCount * COLUMN_WIDTH * zoom;
    const height =
      viewport.height ??
      COLUMN_HEADER_HEIGHT + range.rowCount * ROW_HEIGHT * zoom;
    const dpr = Math.max(1, viewport.devicePixelRatio);
    target.width = Math.max(1, Math.ceil(width * dpr));
    target.height = Math.max(1, Math.ceil(height * dpr));
    if ("style" in target) {
      target.style.width = `${width}px`;
      target.style.height = `${height}px`;
    }
    const context = target.getContext("2d");
    if (!context)
      throw new ViewerError(
        "render-failed",
        "Canvas 2D context is unavailable",
      );
    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawGrid(context, handle.parsed.rows, range, zoom, width, height);
    if (signal?.aborted) throw abortError();
  }

  async getTextMap(
    handle: CsvHandle,
    pageIndex: number,
    signal?: AbortSignal,
  ): Promise<readonly TextRun[]> {
    if (pageIndex !== 0)
      throw new ViewerError("render-failed", "CSV sheet index is out of range");
    const runs: TextRun[] = [];
    for (
      let rowIndex = 0;
      rowIndex < handle.parsed.rows.length;
      rowIndex += 1
    ) {
      if (signal?.aborted) throw abortError();
      const row = handle.parsed.rows[rowIndex]!;
      for (let columnIndex = 0; columnIndex < row.length; columnIndex += 1) {
        const text = row[columnIndex]!;
        if (!text) continue;
        runs.push({
          text,
          x: ROW_HEADER_WIDTH + columnIndex * COLUMN_WIDTH + 6,
          y: COLUMN_HEADER_HEIGHT + rowIndex * ROW_HEIGHT + 4,
          width: COLUMN_WIDTH - 12,
          height: ROW_HEIGHT - 8,
          row: rowIndex + 1,
          column: columnIndex + 1,
          direction: /[\u0590-\u08ff\ufb1d-\ufefc]/u.test(text) ? "rtl" : "ltr",
        });
      }
    }
    return runs;
  }

  close(): void {}
}

export function createCsvAdapter(
  options: CsvAdapterOptions = {},
): CsvDocumentAdapter {
  return new CsvDocumentAdapter(options);
}

function drawGrid(
  context: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D,
  rows: readonly (readonly string[])[],
  range: NonNullable<RenderViewport["sheetRange"]>,
  zoom: number,
  width: number,
  height: number,
): void {
  const rowHeight = ROW_HEIGHT * zoom;
  const columnWidth = COLUMN_WIDTH * zoom;
  context.fillStyle = "#ffffff";
  context.fillRect(0, 0, width, height);
  context.fillStyle = "#f2f4f7";
  context.fillRect(0, 0, width, COLUMN_HEADER_HEIGHT);
  context.fillRect(0, 0, ROW_HEADER_WIDTH, height);
  context.strokeStyle = "#d0d5dd";
  context.lineWidth = 1;
  context.font = `${Math.max(10, 12 * zoom)}px system-ui, sans-serif`;
  context.textBaseline = "middle";

  for (
    let visibleColumn = 0;
    visibleColumn < range.columnCount;
    visibleColumn += 1
  ) {
    const x = ROW_HEADER_WIDTH + visibleColumn * columnWidth;
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, height);
    context.stroke();
    context.fillStyle = "#475467";
    context.fillText(
      columnLabel(range.column + visibleColumn),
      x + 6,
      COLUMN_HEADER_HEIGHT / 2,
    );
  }
  for (let visibleRow = 0; visibleRow < range.rowCount; visibleRow += 1) {
    const rowNumber = range.row + visibleRow;
    const y = COLUMN_HEADER_HEIGHT + visibleRow * rowHeight;
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(width, y);
    context.stroke();
    context.fillStyle = "#475467";
    context.fillText(String(rowNumber), 6, y + rowHeight / 2);
    const row = rows[rowNumber - 1];
    if (!row) continue;
    for (
      let visibleColumn = 0;
      visibleColumn < range.columnCount;
      visibleColumn += 1
    ) {
      const value = row[range.column + visibleColumn - 1];
      if (!value) continue;
      const x = ROW_HEADER_WIDTH + visibleColumn * columnWidth;
      context.save();
      context.beginPath();
      context.rect(x + 3, y + 1, columnWidth - 6, rowHeight - 2);
      context.clip();
      context.fillStyle = "#101828";
      context.direction = /[\u0590-\u08ff\ufb1d-\ufefc]/u.test(value)
        ? "rtl"
        : "ltr";
      context.fillText(value, x + 6, y + rowHeight / 2);
      context.restore();
    }
  }
}

function columnLabel(column: number): string {
  let current = Math.max(1, column);
  let result = "";
  while (current > 0) {
    current -= 1;
    result = String.fromCharCode(65 + (current % 26)) + result;
    current = Math.floor(current / 26);
  }
  return result;
}

function parseInWorker(
  data: Uint8Array,
  format: "csv" | "tsv",
  maxCells: number,
  workerUrl: URL,
  signal: AbortSignal,
): Promise<ParsedDelimitedText> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(workerUrl, {
      type: "module",
      name: "csv-parser",
    });
    const finish = (): void => {
      signal.removeEventListener("abort", onAbort);
      worker.terminate();
    };
    const onAbort = (): void => {
      finish();
      reject(abortError());
    };
    signal.addEventListener("abort", onAbort, { once: true });
    worker.onerror = (event) => {
      finish();
      reject(
        new ViewerError("worker-crashed", "CSV parser worker crashed", {
          details: { message: event.message },
        }),
      );
    };
    worker.onmessage = (
      event: MessageEvent<
        | { ok: true; result: ParsedDelimitedText }
        | { ok: false; code: string; message: string }
      >,
    ) => {
      finish();
      if (event.data.ok) resolve(event.data.result);
      else
        reject(
          new ViewerError(
            event.data.code === "resource-limit"
              ? "resource-limit"
              : "invalid-file",
            event.data.message,
          ),
        );
    };
    const copy = data.slice().buffer;
    worker.postMessage({ data: copy, format, maxCells }, [copy]);
  });
}
