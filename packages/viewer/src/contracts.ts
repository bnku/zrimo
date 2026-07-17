export const supportedFormats = [
  "docx",
  "docm",
  "xlsx",
  "xlsm",
  "pptx",
  "pptm",
  "ppsx",
  "doc",
  "xls",
  "ppt",
  "pdf",
  "csv",
  "tsv",
  "png",
  "jpeg",
  "gif",
  "webp",
  "svg",
  "bmp",
  "tiff",
] as const;

export type DocumentFormat = (typeof supportedFormats)[number];
export type ViewerLocale = "en" | "ru";
export type FitMode = "none" | "page" | "width";
export type ViewerStatus = "idle" | "loading" | "ready" | "error" | "destroyed";
export type RenderUnit = "page" | "slide" | "sheet" | "image";

export type ViewerErrorCode =
  | "unsupported-format"
  | "fidelity-unsupported"
  | "invalid-file"
  | "encrypted-document"
  | "resource-limit"
  | "network-error"
  | "aborted"
  | "font-unavailable"
  | "render-failed"
  | "worker-crashed"
  | "lifecycle-error"
  | "internal";

export type ViewerWarningCode =
  | "format-hint-mismatch"
  | "unsupported-feature"
  | "font-substitution"
  | "font-unavailable"
  | "external-resource-blocked"
  | "fidelity-degraded";

export interface ViewerErrorData {
  readonly name: "ViewerError";
  readonly code: ViewerErrorCode;
  readonly message: string;
  readonly details?: Readonly<Record<string, unknown>>;
}

export interface ViewerWarning {
  readonly code: ViewerWarningCode;
  readonly message: string;
  readonly details?: Readonly<Record<string, unknown>>;
}

export interface ViewerProgress {
  readonly phase:
    "loading" | "detecting" | "parsing" | "converting" | "rendering";
  readonly loaded?: number;
  readonly total?: number;
  readonly ratio?: number;
}

export interface ResourceLimits {
  readonly maxInputBytes: number;
  readonly maxExpandedOfficeBytes: number;
  readonly maxZipEntryBytes: number;
  readonly maxDecodedPixels: number;
  readonly maxSvgBytes: number;
  readonly maxCsvCells: number;
  readonly maxTextMapBytes: number;
  readonly maxDocumentUnits: number;
  readonly maxConcurrentRenders: number;
  readonly maxOperationMs: number;
}

export type BinaryDocumentSource = ArrayBuffer | Uint8Array | Blob;
export type DocumentSource = BinaryDocumentSource | URL | string;

export interface OpenDocumentOptions {
  readonly fileName?: string;
  readonly contentType?: string;
  readonly format?: DocumentFormat;
  readonly signal?: AbortSignal;
  readonly limits?: Partial<ResourceLimits>;
}

export type ViewerFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

export type FontPolicyMode = "auto" | "offline" | "custom";
export type FontStyle = "normal" | "italic" | "oblique";
export type FontScript =
  | "latin"
  | "cyrillic"
  | "cjk"
  | "japanese"
  | "korean"
  | "arabic"
  | "devanagari"
  | "bengali"
  | "gujarati"
  | "gurmukhi"
  | "odia"
  | "tamil"
  | "telugu"
  | "kannada"
  | "malayalam"
  | "unknown";

export type FontBinarySource = ArrayBuffer | Uint8Array;
export type FontSource = FontBinarySource | URL | string;

export interface FontRequest {
  readonly family?: string;
  readonly weight: number;
  readonly style: FontStyle;
  readonly script: FontScript;
  readonly codepoints: readonly number[];
}

export interface FontResolution {
  readonly family?: string;
  readonly source: FontSource;
  readonly weight?: number;
  readonly style?: FontStyle;
}

export type FontResolver = (
  request: FontRequest,
  signal: AbortSignal,
) => FontResolution | null | Promise<FontResolution | null>;

export interface FontPolicy {
  readonly mode?: FontPolicyMode;
  readonly resolver?: FontResolver;
}

export interface RegisteredFont {
  readonly family: string;
  readonly source: FontSource;
  readonly weight?: number;
  readonly style?: FontStyle;
  readonly scripts?: readonly FontScript[];
}

export interface ViewerTranslations {
  readonly previous: string;
  readonly next: string;
  readonly page: string;
  readonly of: string;
  readonly zoomIn: string;
  readonly zoomOut: string;
  readonly fitWidth: string;
  readonly fitPage: string;
  readonly search: string;
  readonly searchPlaceholder: string;
  readonly matches: string;
  readonly thumbnails: string;
  readonly fullscreen: string;
  readonly exitFullscreen: string;
  readonly download: string;
  readonly sheets: string;
  readonly selectedRange: string;
  readonly loading: string;
  readonly close: string;
  readonly noMatches: string;
}

export interface ViewerLogger {
  debug?(message: string, details?: Readonly<Record<string, unknown>>): void;
  warn?(message: string, details?: Readonly<Record<string, unknown>>): void;
  error?(message: string, details?: Readonly<Record<string, unknown>>): void;
}

export interface ViewerOptions {
  readonly container?: HTMLElement;
  readonly locale?: ViewerLocale;
  readonly ui?: boolean;
  readonly initialZoom?: number;
  readonly fit?: FitMode;
  readonly useWorker?: boolean;
  readonly layout?: "continuous" | "single";
  readonly overscan?: number;
  readonly translations?: Partial<ViewerTranslations>;
}

export interface ViewerClientOptions {
  readonly fetch?: ViewerFetch;
  readonly logger?: ViewerLogger;
  readonly assetBaseUrl?: string | URL;
  readonly limits?: Partial<ResourceLimits>;
  readonly adapters?: readonly DocumentAdapter[];
  readonly fontPolicy?: FontPolicy;
  readonly fonts?: readonly RegisteredFont[];
}

export interface ViewerState {
  readonly status: ViewerStatus;
  readonly format?: DocumentFormat;
  readonly pageIndex: number;
  readonly pageCount: number;
  readonly zoom: number;
  readonly fit: FitMode;
  readonly panX: number;
  readonly panY: number;
}

export interface TextRun {
  readonly text: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly direction?: "ltr" | "rtl";
  readonly fontFamily?: string;
  readonly fontWeight?: number;
  readonly fontStyle?: FontStyle;
  /** Exact CSS font shorthand used by a canvas renderer. */
  readonly font?: string;
  /** Font size in the run coordinate space, in CSS pixels. */
  readonly fontSize?: number;
  /** Uniform canvas letter spacing in the run coordinate space. */
  readonly letterSpacingPx?: number;
  /** Renderer-provided CSS transform for rotated/vertical glyph runs. */
  readonly transform?: string;
  /** True for a horizontal-in-vertical (tate-chu-yoko) run. */
  readonly eastAsianVert?: boolean;
  /** Selectable overlay implementation that owns this run's geometry. */
  readonly textLayer?: "docx" | "pdf" | "pptx" | "generic";
  /** Unscaled coordinate-space width/height shared by all runs on a unit. */
  readonly coordinateWidth?: number;
  readonly coordinateHeight?: number;
  /** UTF-16 offsets in the page's logical text stream. */
  readonly logicalStart?: number;
  readonly logicalEnd?: number;
  readonly hyperlink?: HyperlinkTarget;
  readonly row?: number;
  readonly column?: number;
}

export type HyperlinkTarget =
  | { readonly kind: "external"; readonly url: string }
  | {
      readonly kind: "internal";
      readonly ref: string;
      readonly pageIndex?: number;
    };

export interface TextSelection {
  readonly pageIndex: number;
  readonly endPageIndex?: number;
  readonly startOffset?: number;
  readonly endOffset?: number;
  readonly text: string;
  readonly runs: readonly TextRun[];
}

export interface TextSelectionRange {
  readonly startPageIndex: number;
  readonly startOffset: number;
  readonly endPageIndex: number;
  readonly endOffset: number;
}

export interface CellRange {
  readonly sheetIndex: number;
  readonly startRow: number;
  readonly startColumn: number;
  readonly endRow: number;
  readonly endColumn: number;
}

/** A non-contiguous spreadsheet selection, ordered by interaction order. */
export interface CellSelection {
  readonly sheetIndex: number;
  readonly ranges: readonly CellRange[];
}

export interface ViewerEventMap {
  readonly statechange: ViewerState;
  readonly ready: ViewerState;
  readonly error: ViewerErrorData;
  readonly warning: ViewerWarning;
  readonly progress: ViewerProgress;
  readonly selectionchange: TextSelection | CellRange | CellSelection | null;
  readonly pagechange: {
    readonly pageIndex: number;
    readonly pageCount: number;
  };
  readonly zoomchange: { readonly zoom: number; readonly fit: FitMode };
  readonly viewchange: ViewerState;
  readonly searchchange: SearchResult | null;
}

export interface SearchOptions {
  readonly caseSensitive?: boolean;
}

export interface SearchMatch {
  readonly pageIndex: number;
  readonly start: number;
  readonly end: number;
  readonly text: string;
}

export interface SearchResult {
  readonly query: string;
  readonly matches: readonly SearchMatch[];
  readonly activeIndex: number;
}

export interface HeadlessRenderOptions {
  readonly zoom?: number;
  readonly devicePixelRatio?: number;
  readonly width?: number;
  readonly height?: number;
  /** Unscaled pixels hidden before the first rendered spreadsheet column. */
  readonly scrollOffsetX?: number;
  /** Unscaled pixels hidden before the first rendered spreadsheet row. */
  readonly scrollOffsetY?: number;
  /** View-only spreadsheet column widths in unscaled CSS pixels, keyed from 1. */
  readonly columnWidths?: Readonly<Record<number, number>>;
  readonly signal?: AbortSignal;
  readonly priority?: "visible" | "adjacent" | "background";
}

export interface ThumbnailRenderOptions {
  readonly maxWidth?: number;
  readonly maxHeight?: number;
  readonly devicePixelRatio?: number;
  readonly signal?: AbortSignal;
}

export interface RenderViewport {
  readonly pageIndex: number;
  readonly zoom: number;
  readonly devicePixelRatio: number;
  readonly width?: number;
  readonly height?: number;
  readonly scrollOffsetX?: number;
  readonly scrollOffsetY?: number;
  readonly columnWidths?: Readonly<Record<number, number>>;
  readonly sheetRange?: SpreadsheetViewportRange;
}

export interface SpreadsheetViewportRange {
  readonly row: number;
  readonly column: number;
  readonly rowCount: number;
  readonly columnCount: number;
}

export interface SpreadsheetMergedRange {
  readonly startRow: number;
  readonly startColumn: number;
  readonly endRow: number;
  readonly endColumn: number;
}

export interface SpreadsheetSheetInfo {
  readonly name: string;
  readonly frozenRows: number;
  readonly frozenColumns: number;
  readonly mergedRanges: readonly SpreadsheetMergedRange[];
  readonly maxRow: number;
  readonly maxColumn: number;
  /** Geometry is expressed in unscaled CSS pixels and uses 1-based indexes. */
  readonly defaultColumnWidth?: number;
  readonly defaultRowHeight?: number;
  readonly columnWidths?: Readonly<Record<number, number>>;
  readonly rowHeights?: Readonly<Record<number, number>>;
  readonly rowHeaderWidth?: number;
  readonly columnHeaderHeight?: number;
  readonly rightToLeft?: boolean;
}

export interface PageSize {
  /** Natural page width in CSS pixels at zoom 1. */
  readonly width: number;
  /** Natural page height in CSS pixels at zoom 1. */
  readonly height: number;
}

export interface DocumentInfo {
  readonly format: DocumentFormat;
  readonly unit: RenderUnit;
  readonly pageCount: number;
  readonly sheetNames?: readonly string[];
  readonly sheets?: readonly SpreadsheetSheetInfo[];
  /** Natural page geometry, in document order, when exposed by the backend. */
  readonly pageSizes?: readonly PageSize[];
  readonly warnings?: readonly ViewerWarning[];
  readonly capabilities?: DocumentCapabilities;
}

export interface DocumentCapabilities {
  readonly textSelection: boolean;
  readonly cellSelection: boolean;
  readonly search: boolean;
  readonly thumbnails: boolean;
}

export type DocumentMetadata = DocumentInfo;

export interface AdapterOpenContext {
  readonly format: DocumentFormat;
  readonly fileName?: string;
  readonly contentType?: string;
  readonly signal: AbortSignal;
  readonly limits: ResourceLimits;
  readonly assetBaseUrl?: URL;
  readonly reportProgress: (progress: ViewerProgress) => void;
  readonly reportWarning: (warning: ViewerWarning) => void;
}

export interface DocumentAdapter<THandle = unknown> {
  readonly id: string;
  readonly formats: readonly DocumentFormat[];
  open(data: Uint8Array, context: AdapterOpenContext): Promise<THandle>;
  getInfo(handle: THandle): Promise<DocumentInfo>;
  render(
    handle: THandle,
    target: HTMLCanvasElement | OffscreenCanvas,
    viewport: RenderViewport,
    signal?: AbortSignal,
  ): Promise<void>;
  getTextMap?(
    handle: THandle,
    pageIndex: number,
    signal?: AbortSignal,
  ): Promise<readonly TextRun[]>;
  close(handle: THandle): void | Promise<void>;
  destroy?(): void | Promise<void>;
}

export type ViewerEventListener<K extends keyof ViewerEventMap> = (
  event: ViewerEventMap[K],
) => void;

export interface ViewerApi {
  readonly state: ViewerState;
  load(source: DocumentSource, options?: OpenDocumentOptions): Promise<void>;
  open(source: DocumentSource, options?: OpenDocumentOptions): Promise<void>;
  close(): Promise<void>;
  setZoom(zoom: number): void;
  zoomIn(): void;
  zoomOut(): void;
  setFit(mode: FitMode): void;
  fitWidth(): void;
  fitPage(): void;
  panBy(deltaX: number, deltaY: number): void;
  goToPage(pageIndex: number): void;
  next(): void;
  previous(): void;
  setSheet(sheetIndex: number): void;
  renderPage(
    pageIndex: number,
    target: HTMLCanvasElement | OffscreenCanvas,
    options?: HeadlessRenderOptions,
  ): Promise<void>;
  renderThumbnail(
    pageIndex: number,
    target: HTMLCanvasElement | OffscreenCanvas,
    options?: ThumbnailRenderOptions,
  ): Promise<void>;
  renderSheetViewport(
    sheetIndex: number,
    target: HTMLCanvasElement | OffscreenCanvas,
    range: SpreadsheetViewportRange,
    options?: HeadlessRenderOptions,
  ): Promise<void>;
  getPageText(pageIndex: number, signal?: AbortSignal): Promise<string>;
  getDocumentInfo(): DocumentInfo;
  search(query: string, options?: SearchOptions): Promise<SearchResult>;
  searchNext(): SearchResult | null;
  searchPrevious(): SearchResult | null;
  clearSearch(): void;
  getSelection(): TextSelection | CellRange | CellSelection | null;
  selectText(range: TextSelectionRange): Promise<TextSelection>;
  selectCells(range: CellRange): CellRange;
  selectCellRanges(ranges: readonly CellRange[]): CellSelection;
  clearSelection(): void;
  copySelection(): Promise<string>;
  getOriginalBytes(): Uint8Array | undefined;
  downloadOriginal(fileName?: string): Blob;
  on<K extends keyof ViewerEventMap>(
    type: K,
    listener: ViewerEventListener<K>,
  ): () => void;
  destroy(): Promise<void>;
}
