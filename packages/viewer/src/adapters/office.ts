import type {
  AdapterOpenContext,
  DocumentAdapter,
  DocumentFormat,
  DocumentInfo,
  HyperlinkTarget,
  RenderViewport,
  SpreadsheetSheetInfo,
  TextRun,
  ViewerWarning,
} from "../contracts.js";
import { abortError, ViewerError } from "../errors.js";
import { enforceContainerLimits } from "../limits.js";

const MODERN_FORMATS = [
  "docx",
  "docm",
  "xlsx",
  "xlsm",
  "pptx",
  "pptm",
  "ppsx",
] as const;
const LEGACY_FORMATS = ["doc", "xls", "ppt"] as const;
const DEFAULT_COLUMN_WIDTH = 64;
const DEFAULT_ROW_HEIGHT = 20;
const ROW_HEADER_WIDTH = 50;
const COLUMN_HEADER_HEIGHT = 22;

type ModernFormat = (typeof MODERN_FORMATS)[number];
type LegacyFormat = (typeof LEGACY_FORMATS)[number];
type OfficeKind = "document" | "spreadsheet" | "presentation";

interface EngineLoadOptions {
  readonly useGoogleFonts: false;
  readonly maxZipEntryBytes: number;
  readonly mode: "main";
}

interface EngineHyperlink {
  readonly kind: "external" | "internal";
  readonly url?: string;
  readonly ref?: string;
  readonly slideIndex?: number;
}

interface DocxRun {
  readonly text: string;
  readonly x: number;
  readonly y: number;
  readonly w: number;
  readonly h: number;
  readonly fontSize: number;
  readonly font: string;
  readonly letterSpacingPx?: number;
  readonly transform?: string;
  readonly eastAsianVert?: boolean;
  readonly hyperlink?: EngineHyperlink;
}

interface DocxBackend {
  readonly pageCount: number;
  pageSize(pageIndex: number): { widthPt: number; heightPt: number };
  renderPage(
    target: HTMLCanvasElement | OffscreenCanvas,
    pageIndex: number,
    options: { width: number; dpr: number },
  ): Promise<void>;
  collectPageRuns(
    pageIndex: number,
    options?: { width?: number; dpr?: number },
  ): Promise<readonly DocxRun[]>;
  getBookmarkPage?(bookmark: string): number | undefined;
  destroy(): void;
}

interface PptxRun {
  readonly text: string;
  readonly inShapeX: number;
  readonly inShapeY: number;
  readonly w: number;
  readonly h: number;
  readonly shapeX: number;
  readonly shapeY: number;
  readonly hyperlink?: EngineHyperlink;
}

interface PptxBackend {
  readonly slideCount: number;
  readonly slideWidth: number;
  readonly slideHeight: number;
  renderSlide(
    target: HTMLCanvasElement | OffscreenCanvas,
    slideIndex: number,
    options: { width: number; dpr: number },
  ): Promise<void>;
  collectSlideRuns(
    slideIndex: number,
    width?: number,
  ): Promise<readonly PptxRun[]>;
  resolveInternalTarget?(
    ref: string,
    currentIndex?: number,
  ): number | undefined;
  destroy(): void;
}

type CellValue =
  | { type: "empty" }
  | { type: "text"; text: string }
  | { type: "number"; number: number }
  | { type: "bool"; bool: boolean }
  | { type: "error"; error: string }
  | { type: "shared"; si: number };

interface SpreadsheetCell {
  readonly row: number;
  readonly col: number;
  value: CellValue;
  formula?: string;
}

interface SpreadsheetWorksheet {
  readonly name: string;
  readonly rows: readonly {
    readonly index: number;
    readonly height?: number | null;
    readonly hidden?: boolean;
    readonly cells: readonly SpreadsheetCell[];
  }[];
  readonly colWidths?: Readonly<Record<number, number>>;
  readonly rowHeights?: Readonly<Record<number, number>>;
  readonly colHidden?: Readonly<Record<number, boolean>>;
  readonly defaultColWidth?: number;
  readonly defaultRowHeight?: number;
  readonly defaultFontFamily?: string;
  readonly defaultFontSize?: number;
  readonly mergeCells: readonly {
    readonly top: number;
    readonly left: number;
    readonly bottom: number;
    readonly right: number;
  }[];
  readonly freezeRows: number;
  readonly freezeCols: number;
  readonly rightToLeft?: boolean;
  readonly images?: readonly SpreadsheetDrawingAnchor[];
  readonly charts?: readonly SpreadsheetDrawingAnchor[];
  readonly hyperlinks?: readonly {
    readonly row: number;
    readonly col: number;
    readonly url: string | null;
    readonly location?: string | null;
  }[];
  readonly parseError?: string;
}

interface SpreadsheetDrawingAnchor {
  readonly fromCol: number;
  readonly fromRow: number;
  readonly toCol: number;
  readonly toRow: number;
}

interface XlsxRun {
  readonly text: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly row: number;
  readonly col: number;
}

interface XlsxBackend {
  readonly sheetNames: readonly string[];
  readonly sheetCount: number;
  getWorksheet(sheetIndex: number): Promise<SpreadsheetWorksheet>;
  cellText?(worksheet: SpreadsheetWorksheet, cell: SpreadsheetCell): string;
  renderViewport(
    target: HTMLCanvasElement | OffscreenCanvas,
    sheetIndex: number,
    range: { row: number; col: number; rows: number; cols: number },
    options: {
      width: number;
      height: number;
      dpr: number;
      cellScale: number;
      freezeRows?: number;
      freezeCols?: number;
      scrollOffsetX?: number;
      scrollOffsetY?: number;
      onTextRun?: (run: XlsxRun) => void;
    },
  ): Promise<void>;
  destroy(): void;
}

export interface OfficeEngineLoaders {
  readonly docx?: (
    data: ArrayBuffer,
    options: EngineLoadOptions,
  ) => Promise<DocxBackend>;
  readonly xlsx?: (
    data: ArrayBuffer,
    options: EngineLoadOptions,
  ) => Promise<XlsxBackend>;
  readonly pptx?: (
    data: ArrayBuffer,
    options: EngineLoadOptions,
  ) => Promise<PptxBackend>;
}

export interface LegacyConversionOptions {
  readonly workerUrl?: string | URL;
  readonly moduleUrl?: string | URL;
  readonly convert?: (
    data: Uint8Array,
    format: LegacyFormat,
    context: AdapterOpenContext,
  ) => Promise<Uint8Array>;
}

export interface OfficeAdapterOptions {
  readonly engines?: OfficeEngineLoaders;
  readonly legacy?: LegacyConversionOptions;
}

interface DocumentHandle {
  readonly kind: "document";
  readonly format: DocumentFormat;
  readonly backend: DocxBackend;
  readonly warnings: readonly ViewerWarning[];
}

interface PresentationHandle {
  readonly kind: "presentation";
  readonly format: DocumentFormat;
  readonly backend: PptxBackend;
  readonly warnings: readonly ViewerWarning[];
}

interface SpreadsheetHandle {
  readonly kind: "spreadsheet";
  readonly format: DocumentFormat;
  readonly backend: XlsxBackend;
  readonly warnings: readonly ViewerWarning[];
  readonly worksheets: readonly SpreadsheetWorksheet[];
  readonly sheets: readonly SpreadsheetSheetInfo[];
}

type OfficeHandle = DocumentHandle | PresentationHandle | SpreadsheetHandle;

export class OfficeDocumentAdapter implements DocumentAdapter<OfficeHandle> {
  readonly id = "office";
  readonly formats = [...MODERN_FORMATS, ...LEGACY_FORMATS] as const;
  readonly #options: OfficeAdapterOptions;

  constructor(options: OfficeAdapterOptions = {}) {
    this.#options = options;
  }

  async open(
    input: Uint8Array,
    context: AdapterOpenContext,
  ): Promise<OfficeHandle> {
    throwIfAborted(context.signal);
    let format = context.format;
    let data = input;
    const warnings: ViewerWarning[] = [];

    if (isLegacyFormat(format)) {
      if (format === "doc")
        throw new ViewerError(
          "fidelity-unsupported",
          "Legacy DOC rendering is disabled because the available browser parser cannot preserve source formatting, sections, or tables",
          {
            details: {
              format,
              capability: "structured-word-binary",
              fallback: "extractLegacyPlainText",
            },
          },
        );
      context.reportProgress({ phase: "converting", loaded: 0, total: 1 });
      data = await this.#convertLegacy(input, format, context);
      throwIfAborted(context.signal);
      const modern = modernFormatFor(format);
      enforceContainerLimits(data, modern, context.limits);
      warnings.push({
        code: "fidelity-degraded",
        message: `${format.toUpperCase()} was converted in memory to ${modern.toUpperCase()}; unsupported legacy features may differ`,
        details: { sourceFormat: format, normalizedFormat: modern },
      });
      format = modern;
      context.reportProgress({
        phase: "converting",
        loaded: 1,
        total: 1,
        ratio: 1,
      });
    }

    if (isMacroFormat(context.format))
      warnings.push({
        code: "unsupported-feature",
        message: "VBA content is ignored and is never executed",
        details: { feature: "vba", format: context.format },
      });

    context.reportProgress({ phase: "parsing", loaded: 0, total: 1 });
    const engineOptions: EngineLoadOptions = {
      useGoogleFonts: false,
      maxZipEntryBytes: context.limits.maxZipEntryBytes,
      mode: "main",
    };
    const buffer = exactArrayBuffer(data);

    try {
      const kind = kindFor(format);
      if (kind === "document") {
        const backend = await this.#loadDocx(buffer, engineOptions);
        throwIfAborted(context.signal, backend);
        context.reportProgress({
          phase: "parsing",
          loaded: 1,
          total: 1,
          ratio: 1,
        });
        return { kind, format: context.format, backend, warnings };
      }
      if (kind === "presentation") {
        const backend = await this.#loadPptx(buffer, engineOptions);
        throwIfAborted(context.signal, backend);
        context.reportProgress({
          phase: "parsing",
          loaded: 1,
          total: 1,
          ratio: 1,
        });
        return { kind, format: context.format, backend, warnings };
      }

      const backend = await this.#loadXlsx(buffer, engineOptions);
      throwIfAborted(context.signal, backend);
      const worksheets: SpreadsheetWorksheet[] = [];
      const sheets: SpreadsheetSheetInfo[] = [];
      let missingFormulaCaches = 0;
      for (
        let sheetIndex = 0;
        sheetIndex < backend.sheetCount;
        sheetIndex += 1
      ) {
        throwIfAborted(context.signal, backend);
        const worksheet = await backend.getWorksheet(sheetIndex);
        const normalized = normalizeWorksheet(worksheet);
        missingFormulaCaches += normalized.missingFormulaCaches;
        worksheets.push(worksheet);
        sheets.push(normalized.info);
        if (worksheet.parseError)
          warnings.push({
            code: "fidelity-degraded",
            message: `Sheet ${worksheet.name} was only partially parsed`,
            details: { sheetIndex, reason: worksheet.parseError },
          });
      }
      if (missingFormulaCaches > 0)
        warnings.push({
          code: "fidelity-degraded",
          message: `${missingFormulaCaches} formula cell(s) had no cached result; formula text is shown without calculation`,
          details: { feature: "formula-cache", count: missingFormulaCaches },
        });
      context.reportProgress({
        phase: "parsing",
        loaded: 1,
        total: 1,
        ratio: 1,
      });
      return {
        kind,
        format: context.format,
        backend,
        warnings,
        worksheets,
        sheets,
      };
    } catch (error) {
      throw normalizeOfficeError(error);
    }
  }

  async getInfo(handle: OfficeHandle): Promise<DocumentInfo> {
    if (handle.kind === "document")
      return {
        format: handle.format,
        unit: "page",
        pageCount: handle.backend.pageCount,
        warnings: handle.warnings,
      };
    if (handle.kind === "presentation")
      return {
        format: handle.format,
        unit: "slide",
        pageCount: handle.backend.slideCount,
        warnings: handle.warnings,
      };
    return {
      format: handle.format,
      unit: "sheet",
      pageCount: handle.backend.sheetCount,
      sheetNames: handle.backend.sheetNames,
      sheets: handle.sheets,
      warnings: handle.warnings,
    };
  }

  async render(
    handle: OfficeHandle,
    target: HTMLCanvasElement | OffscreenCanvas,
    viewport: RenderViewport,
    signal?: AbortSignal,
  ): Promise<void> {
    throwIfAborted(signal);
    assertUnitIndex(viewport.pageIndex, await this.getInfo(handle));
    try {
      if (handle.kind === "document") {
        const page = handle.backend.pageSize(viewport.pageIndex);
        await handle.backend.renderPage(target, viewport.pageIndex, {
          width: (page.widthPt * 96 * viewport.zoom) / 72,
          dpr: viewport.devicePixelRatio,
        });
      } else if (handle.kind === "presentation") {
        await handle.backend.renderSlide(target, viewport.pageIndex, {
          width: (handle.backend.slideWidth / 9525) * viewport.zoom,
          dpr: viewport.devicePixelRatio,
        });
      } else {
        const range = normalizeSheetRange(viewport.sheetRange);
        const worksheet = handle.worksheets[viewport.pageIndex]!;
        await handle.backend.renderViewport(
          target,
          viewport.pageIndex,
          {
            row: range.row,
            col: range.column,
            rows: range.rowCount,
            cols: range.columnCount,
          },
          {
            width:
              viewport.width ??
              canvasCssWidth(target, viewport.devicePixelRatio),
            height:
              viewport.height ??
              canvasCssHeight(target, viewport.devicePixelRatio),
            dpr: viewport.devicePixelRatio,
            cellScale: viewport.zoom,
            freezeRows: worksheet.freezeRows,
            freezeCols: worksheet.freezeCols,
            ...(viewport.scrollOffsetX === undefined
              ? {}
              : { scrollOffsetX: viewport.scrollOffsetX }),
            ...(viewport.scrollOffsetY === undefined
              ? {}
              : { scrollOffsetY: viewport.scrollOffsetY }),
          },
        );
      }
      throwIfAborted(signal);
    } catch (error) {
      if (signal?.aborted) throw abortError();
      throw error instanceof ViewerError
        ? error
        : new ViewerError("render-failed", "Office rendering failed", {
            cause: error,
          });
    }
  }

  async getTextMap(
    handle: OfficeHandle,
    pageIndex: number,
    signal?: AbortSignal,
  ): Promise<readonly TextRun[]> {
    throwIfAborted(signal);
    assertUnitIndex(pageIndex, await this.getInfo(handle));
    if (handle.kind === "document") {
      const size = handle.backend.pageSize(pageIndex);
      const coordinateWidth = (size.widthPt * 96) / 72;
      const coordinateHeight = (size.heightPt * 96) / 72;
      const runs = await handle.backend.collectPageRuns(pageIndex, {
        width: coordinateWidth,
        dpr: 1,
      });
      let logicalOffset = 0;
      return runs.map((run) => {
        const logicalStart = logicalOffset;
        logicalOffset += run.text.length;
        return {
          text: run.text,
          x: run.x,
          y: run.y,
          width: run.w,
          height: run.h,
          font: run.font,
          fontSize: run.fontSize,
          ...(run.letterSpacingPx === undefined
            ? {}
            : { letterSpacingPx: run.letterSpacingPx }),
          ...(run.transform === undefined ? {} : { transform: run.transform }),
          ...(run.eastAsianVert === undefined
            ? {}
            : { eastAsianVert: run.eastAsianVert }),
          textLayer: "docx" as const,
          coordinateWidth,
          coordinateHeight,
          logicalStart,
          logicalEnd: logicalOffset,
          ...fontProperties(run.font),
          ...safeHyperlink(run.hyperlink, (ref) =>
            handle.backend.getBookmarkPage?.(ref),
          ),
          direction: textDirection(run.text),
        };
      });
    }
    if (handle.kind === "presentation") {
      const width = handle.backend.slideWidth / 9525;
      const runs = await handle.backend.collectSlideRuns(pageIndex, width);
      return runs.map((run) => ({
        text: run.text,
        x: run.shapeX + run.inShapeX,
        y: run.shapeY + run.inShapeY,
        width: run.w,
        height: run.h,
        ...safeHyperlink(run.hyperlink, (ref) =>
          handle.backend.resolveInternalTarget?.(ref, pageIndex),
        ),
        direction: textDirection(run.text),
      }));
    }

    const worksheet = handle.worksheets[pageIndex]!;
    throwIfAborted(signal);
    const info = handle.sheets[pageIndex]!;
    const hyperlinks = spreadsheetHyperlinks(worksheet);
    const result: TextRun[] = [];
    for (const row of worksheet.rows) {
      for (const cell of row.cells) {
        throwIfAborted(signal);
        const text =
          handle.backend.cellText?.(worksheet, cell) ?? cellText(cell);
        if (!text) continue;
        const width = axisSize(
          cell.col,
          info.defaultColumnWidth ?? DEFAULT_COLUMN_WIDTH,
          info.columnWidths,
        );
        const height = axisSize(
          cell.row,
          info.defaultRowHeight ?? DEFAULT_ROW_HEIGHT,
          info.rowHeights,
        );
        result.push({
          text,
          x:
            (info.rowHeaderWidth ?? ROW_HEADER_WIDTH) +
            axisOffset(
              cell.col,
              info.defaultColumnWidth ?? DEFAULT_COLUMN_WIDTH,
              info.columnWidths,
            ) +
            4,
          y:
            (info.columnHeaderHeight ?? COLUMN_HEADER_HEIGHT) +
            axisOffset(
              cell.row,
              info.defaultRowHeight ?? DEFAULT_ROW_HEIGHT,
              info.rowHeights,
            ) +
            2,
          width: Math.max(0, width - 8),
          height: Math.max(0, height - 4),
          row: cell.row,
          column: cell.col,
          ...(hyperlinks.get(`${cell.row}:${cell.col}`)
            ? { hyperlink: hyperlinks.get(`${cell.row}:${cell.col}`)! }
            : {}),
          direction: textDirection(text),
        });
      }
    }
    return result;
  }

  close(handle: OfficeHandle): void {
    handle.backend.destroy();
  }

  async #convertLegacy(
    data: Uint8Array,
    format: LegacyFormat,
    context: AdapterOpenContext,
  ): Promise<Uint8Array> {
    if (this.#options.legacy?.convert)
      return this.#options.legacy.convert(data, format, context);
    if (typeof Worker === "undefined")
      throw new ViewerError(
        "internal",
        "Legacy Office conversion requires a browser Worker or an injected converter",
      );
    const workerUrl = this.#options.legacy?.workerUrl
      ? new URL(this.#options.legacy.workerUrl, context.assetBaseUrl)
      : context.assetBaseUrl
        ? new URL("workers/legacy-converter-worker.js", context.assetBaseUrl)
        : new URL("../legacy-converter-worker.js", import.meta.url);
    const moduleUrl = this.#options.legacy?.moduleUrl
      ? new URL(this.#options.legacy.moduleUrl, context.assetBaseUrl)
      : context.assetBaseUrl
        ? new URL("wasm/legacy/index.js", context.assetBaseUrl)
        : new URL("../assets/legacy/index.js", import.meta.url);
    return convertInWorker(
      data,
      format,
      workerUrl,
      moduleUrl,
      context.signal,
      context.limits.maxOperationMs,
    );
  }

  async #loadDocx(
    data: ArrayBuffer,
    options: EngineLoadOptions,
  ): Promise<DocxBackend> {
    if (this.#options.engines?.docx)
      return this.#options.engines.docx(data, options);
    const { DocxDocument } = await import("@silurus/ooxml/docx");
    return DocxDocument.load(data, options);
  }

  async #loadXlsx(
    data: ArrayBuffer,
    options: EngineLoadOptions,
  ): Promise<XlsxBackend> {
    if (this.#options.engines?.xlsx)
      return this.#options.engines.xlsx(data, options);
    const { XlsxWorkbook } = await import("@silurus/ooxml/xlsx");
    return XlsxWorkbook.load(data, options);
  }

  async #loadPptx(
    data: ArrayBuffer,
    options: EngineLoadOptions,
  ): Promise<PptxBackend> {
    if (this.#options.engines?.pptx)
      return this.#options.engines.pptx(data, options);
    const { PptxPresentation } = await import("@silurus/ooxml/pptx");
    return PptxPresentation.load(data, options);
  }
}

export function createOfficeAdapter(
  options: OfficeAdapterOptions = {},
): OfficeDocumentAdapter {
  return new OfficeDocumentAdapter(options);
}

export function sanitizeOfficeHyperlink(
  target: EngineHyperlink | undefined,
): HyperlinkTarget | undefined {
  if (!target) return undefined;
  if (target.kind === "internal" && target.ref)
    return {
      kind: "internal",
      ref: target.ref,
      ...(target.slideIndex === undefined
        ? {}
        : { pageIndex: target.slideIndex }),
    };
  if (target.kind !== "external" || !target.url) return undefined;
  try {
    const url = new URL(target.url);
    if (!["http:", "https:", "mailto:", "tel:"].includes(url.protocol))
      return undefined;
    return { kind: "external", url: url.href };
  } catch {
    return undefined;
  }
}

function safeHyperlink(
  target: EngineHyperlink | undefined,
  resolveInternal: (ref: string) => number | undefined,
): { hyperlink?: HyperlinkTarget } {
  const sanitized = sanitizeOfficeHyperlink(target);
  if (!sanitized) return {};
  if (sanitized.kind === "external") return { hyperlink: sanitized };
  const pageIndex = sanitized.pageIndex ?? resolveInternal(sanitized.ref);
  return {
    hyperlink: {
      ...sanitized,
      ...(pageIndex === undefined ? {} : { pageIndex }),
    },
  };
}

function fontProperties(font: string): {
  fontFamily?: string;
  fontWeight?: number;
  fontStyle?: "normal" | "italic" | "oblique";
} {
  const family = font.match(/\b\d+(?:\.\d+)?px(?:\/[^\s]+)?\s+(.+)$/)?.[1];
  const weight = font.match(/(?:^|\s)([1-9]00|bold)(?=\s)/)?.[1];
  const style = font.match(/(?:^|\s)(italic|oblique)(?=\s)/)?.[1] as
    "italic" | "oblique" | undefined;
  return {
    ...(family
      ? {
          fontFamily: family
            .split(",", 1)[0]!
            .trim()
            .replace(/^['"]|['"]$/g, ""),
        }
      : {}),
    ...(weight ? { fontWeight: weight === "bold" ? 700 : Number(weight) } : {}),
    ...(style ? { fontStyle: style } : {}),
  };
}

function normalizeWorksheet(worksheet: SpreadsheetWorksheet): {
  info: SpreadsheetSheetInfo;
  missingFormulaCaches: number;
} {
  let missingFormulaCaches = 0;
  let maxRow = 0;
  let maxColumn = 0;
  for (const row of worksheet.rows) {
    maxRow = Math.max(maxRow, row.index);
    for (const cell of row.cells) {
      maxRow = Math.max(maxRow, cell.row);
      maxColumn = Math.max(maxColumn, cell.col);
      if (cell.formula === undefined) continue;
      if (cell.value.type === "empty") {
        missingFormulaCaches += 1;
        cell.value = { type: "text", text: `=${cell.formula}` };
      }
      delete cell.formula;
    }
  }
  for (const range of worksheet.mergeCells) {
    maxRow = Math.max(maxRow, range.bottom);
    maxColumn = Math.max(maxColumn, range.right);
  }
  for (const drawing of [
    ...(worksheet.images ?? []),
    ...(worksheet.charts ?? []),
  ]) {
    maxRow = Math.max(maxRow, drawing.fromRow + 1, drawing.toRow + 1);
    maxColumn = Math.max(maxColumn, drawing.fromCol + 1, drawing.toCol + 1);
  }
  const defaultColumnWidth = columnWidthToPixels(
    worksheet.defaultColWidth ?? 8.43,
  );
  const defaultRowHeight = pointsToPixels(worksheet.defaultRowHeight ?? 15);
  const columnWidths: Record<number, number> = {};
  for (const [rawIndex, rawWidth] of Object.entries(
    worksheet.colWidths ?? {},
  )) {
    const index = Number(rawIndex);
    if (!Number.isInteger(index) || index < 1) continue;
    columnWidths[index] = worksheet.colHidden?.[index]
      ? 0
      : columnWidthToPixels(rawWidth);
    maxColumn = Math.max(maxColumn, index);
  }
  for (const [rawIndex, hidden] of Object.entries(worksheet.colHidden ?? {})) {
    const index = Number(rawIndex);
    if (!hidden || !Number.isInteger(index) || index < 1) continue;
    columnWidths[index] = 0;
    maxColumn = Math.max(maxColumn, index);
  }
  const rowHeights: Record<number, number> = {};
  for (const [rawIndex, rawHeight] of Object.entries(
    worksheet.rowHeights ?? {},
  )) {
    const index = Number(rawIndex);
    if (!Number.isInteger(index) || index < 1) continue;
    rowHeights[index] = pointsToPixels(rawHeight);
    maxRow = Math.max(maxRow, index);
  }
  for (const row of worksheet.rows) {
    if (row.hidden) rowHeights[row.index] = 0;
    else if (row.height !== undefined && row.height !== null)
      rowHeights[row.index] = pointsToPixels(row.height);
  }
  return {
    missingFormulaCaches,
    info: {
      name: worksheet.name,
      frozenRows: worksheet.freezeRows,
      frozenColumns: worksheet.freezeCols,
      mergedRanges: worksheet.mergeCells.map((range) => ({
        startRow: range.top,
        startColumn: range.left,
        endRow: range.bottom,
        endColumn: range.right,
      })),
      maxRow,
      maxColumn,
      defaultColumnWidth,
      defaultRowHeight,
      columnWidths,
      rowHeights,
      rowHeaderWidth: ROW_HEADER_WIDTH,
      columnHeaderHeight: COLUMN_HEADER_HEIGHT,
      rightToLeft: worksheet.rightToLeft ?? false,
    },
  };
}

function pointsToPixels(points: number): number {
  return Math.max(0, Math.round(Math.max(0, points) * (4 / 3)));
}

function columnWidthToPixels(width: number): number {
  const maximumDigitWidth = 8;
  const safeWidth = Math.max(0, Number.isFinite(width) ? width : 0);
  return Math.max(
    0,
    Math.trunc(
      ((256 * safeWidth + Math.trunc(128 / maximumDigitWidth)) / 256) *
        maximumDigitWidth,
    ),
  );
}

function axisSize(
  index: number,
  defaultSize: number,
  overrides?: Readonly<Record<number, number>>,
): number {
  return overrides?.[index] ?? defaultSize;
}

function axisOffset(
  index: number,
  defaultSize: number,
  overrides?: Readonly<Record<number, number>>,
): number {
  const before = Math.max(0, index - 1);
  let offset = before * defaultSize;
  for (const [rawIndex, size] of Object.entries(overrides ?? {})) {
    const overrideIndex = Number(rawIndex);
    if (overrideIndex >= 1 && overrideIndex < index)
      offset += size - defaultSize;
  }
  return offset;
}

function cellText(cell: SpreadsheetCell): string {
  switch (cell.value.type) {
    case "text":
      return cell.value.text;
    case "number":
      return String(cell.value.number);
    case "bool":
      return cell.value.bool ? "TRUE" : "FALSE";
    case "error":
      return cell.value.error;
    default:
      return "";
  }
}

function spreadsheetHyperlinks(
  worksheet: SpreadsheetWorksheet,
): ReadonlyMap<string, HyperlinkTarget> {
  const result = new Map<string, HyperlinkTarget>();
  for (const link of worksheet.hyperlinks ?? []) {
    const target = link.location
      ? ({ kind: "internal", ref: link.location } as const)
      : link.url
        ? ({ kind: "external", url: link.url } as const)
        : undefined;
    const sanitized = sanitizeOfficeHyperlink(target);
    if (sanitized) result.set(`${link.row}:${link.col}`, sanitized);
  }
  return result;
}

function convertInWorker(
  data: Uint8Array,
  format: LegacyFormat,
  workerUrl: URL,
  moduleUrl: URL,
  signal: AbortSignal,
  timeoutMs: number,
): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(workerUrl, {
      type: "module",
      name: "legacy-office",
    });
    const finish = (): void => {
      clearTimeout(timeout);
      signal.removeEventListener("abort", onAbort);
      worker.terminate();
    };
    const onAbort = (): void => {
      finish();
      reject(abortError());
    };
    const timeout = setTimeout(() => {
      finish();
      reject(
        new ViewerError(
          "resource-limit",
          "Legacy Office conversion exceeded maxOperationMs",
          { details: { format, timeoutMs } },
        ),
      );
    }, timeoutMs);
    signal.addEventListener("abort", onAbort, { once: true });
    worker.onerror = (event) => {
      finish();
      reject(
        new ViewerError("worker-crashed", "Legacy conversion worker crashed", {
          details: { message: event.message },
        }),
      );
    };
    worker.onmessage = (event: MessageEvent<LegacyWorkerResponse>) => {
      finish();
      if (event.data.ok) resolve(new Uint8Array(event.data.data));
      else
        reject(
          new ViewerError("invalid-file", "Legacy Office conversion failed", {
            details: { reason: event.data.message, format },
          }),
        );
    };
    const copy = data.slice().buffer;
    worker.postMessage({ data: copy, format, moduleUrl: moduleUrl.href }, [
      copy,
    ]);
  });
}

interface LegacyWorkerResponse {
  readonly ok: boolean;
  readonly data: ArrayBuffer;
  readonly message?: string;
}

function makeTextMapCanvas(): HTMLCanvasElement | OffscreenCanvas {
  if (typeof OffscreenCanvas !== "undefined") return new OffscreenCanvas(1, 1);
  if (typeof document !== "undefined") return document.createElement("canvas");
  throw new ViewerError(
    "render-failed",
    "Text-map collection requires OffscreenCanvas or a DOM canvas",
  );
}

function canvasCssWidth(
  target: HTMLCanvasElement | OffscreenCanvas,
  dpr: number,
): number {
  return "clientWidth" in target && target.clientWidth > 0
    ? target.clientWidth
    : Math.max(1, target.width / Math.max(1, dpr));
}

function canvasCssHeight(
  target: HTMLCanvasElement | OffscreenCanvas,
  dpr: number,
): number {
  return "clientHeight" in target && target.clientHeight > 0
    ? target.clientHeight
    : Math.max(1, target.height / Math.max(1, dpr));
}

function normalizeSheetRange(
  range: RenderViewport["sheetRange"],
): NonNullable<RenderViewport["sheetRange"]> {
  return range
    ? {
        row: Math.max(1, Math.trunc(range.row)),
        column: Math.max(1, Math.trunc(range.column)),
        rowCount: Math.max(1, Math.trunc(range.rowCount)),
        columnCount: Math.max(1, Math.trunc(range.columnCount)),
      }
    : { row: 1, column: 1, rowCount: 100, columnCount: 30 };
}

function assertUnitIndex(index: number, info: DocumentInfo): void {
  if (!Number.isInteger(index) || index < 0 || index >= info.pageCount)
    throw new ViewerError(
      "render-failed",
      "Office unit index is out of range",
      {
        details: { index, pageCount: info.pageCount, unit: info.unit },
      },
    );
}

function throwIfAborted(
  signal?: AbortSignal,
  backend?: { destroy(): void },
): void {
  if (!signal?.aborted) return;
  backend?.destroy();
  throw abortError();
}

function exactArrayBuffer(data: Uint8Array): ArrayBuffer {
  return data.slice().buffer;
}

function isLegacyFormat(format: DocumentFormat): format is LegacyFormat {
  return (LEGACY_FORMATS as readonly string[]).includes(format);
}

function isMacroFormat(format: DocumentFormat): boolean {
  return format === "docm" || format === "xlsm" || format === "pptm";
}

function modernFormatFor(format: LegacyFormat): ModernFormat {
  if (format === "doc") return "docx";
  if (format === "xls") return "xlsx";
  return "pptx";
}

function kindFor(format: DocumentFormat): OfficeKind {
  if (format === "docx" || format === "docm") return "document";
  if (format === "xlsx" || format === "xlsm") return "spreadsheet";
  return "presentation";
}

function normalizeOfficeError(error: unknown): ViewerError {
  if (error instanceof ViewerError) return error;
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? String(error.code)
      : undefined;
  if (
    code === "encrypted" ||
    code === "invalid-password" ||
    code === "unsupported-encryption"
  )
    return new ViewerError(
      "encrypted-document",
      "Password-protected Office documents are not supported",
      { cause: error, details: { backendCode: code } },
    );
  return new ViewerError(
    "invalid-file",
    error instanceof Error ? error.message : "Invalid Office document",
    { cause: error, ...(code ? { details: { backendCode: code } } : {}) },
  );
}

function textDirection(text: string): "ltr" | "rtl" {
  return /[\u0590-\u08ff\ufb1d-\ufefc]/u.test(text) ? "rtl" : "ltr";
}
