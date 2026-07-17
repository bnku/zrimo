import type {
  CellRange,
  DocumentInfo,
  HeadlessRenderOptions,
  SearchMatch,
  TextRun,
  TextSelectionRange,
  ViewerState,
  SpreadsheetViewportRange,
} from "./contracts.js";
import { snapGraphemeOffset } from "./interaction.js";
import { SpreadsheetViewport } from "./spreadsheet-viewport.js";

import type { DocxHighlightMatch, DocxTextRunInfo } from "@silurus/ooxml/docx";

const BASE_WIDTH = 816;
const BASE_HEIGHT = 1056;
const PAGE_GAP = 24;

export interface ViewportHost {
  readonly state: ViewerState;
  renderPage(
    pageIndex: number,
    target: HTMLCanvasElement,
    options: HeadlessRenderOptions,
  ): Promise<void>;
  renderSheetViewport(
    sheetIndex: number,
    target: HTMLCanvasElement,
    range: SpreadsheetViewportRange,
    options: HeadlessRenderOptions,
  ): Promise<void>;
  getTextRuns(
    pageIndex: number,
    signal?: AbortSignal,
  ): Promise<readonly TextRun[]>;
  getSearchMatches(pageIndex: number): readonly SearchMatch[];
  onVisiblePage(pageIndex: number): void;
  onPan(panX: number, panY: number): void;
  onZoom(zoom: number): void;
  onTextSelection(range: TextSelectionRange): void;
  onCellSelection(range: CellRange): void;
  onCopySelection(): void;
}

interface ViewportStrategy {
  setDocument(info: DocumentInfo | undefined): void;
  update(): void;
  panBy(deltaX: number, deltaY: number): void;
  goToPage(pageIndex: number): void;
  fitWidth(): number;
  fitPage(): number;
  destroy(): void;
}

export class AdaptiveViewport implements ViewportStrategy {
  readonly #container: HTMLElement;
  readonly #host: ViewportHost;
  readonly #options: {
    readonly overscan?: number;
    readonly layout?: "continuous" | "single";
  };
  #strategy: ViewportStrategy;
  #kind: "page" | "sheet" = "page";

  constructor(
    container: HTMLElement,
    host: ViewportHost,
    options: {
      readonly overscan?: number;
      readonly layout?: "continuous" | "single";
    },
  ) {
    this.#container = container;
    this.#host = host;
    this.#options = options;
    this.#strategy = new ViewerViewport(container, host, options);
  }

  setDocument(info: DocumentInfo | undefined): void {
    const nextKind = info?.unit === "sheet" ? "sheet" : "page";
    if (nextKind !== this.#kind) {
      this.#strategy.destroy();
      this.#kind = nextKind;
      this.#strategy =
        nextKind === "sheet"
          ? new SpreadsheetViewport(this.#container, this.#host)
          : new ViewerViewport(this.#container, this.#host, this.#options);
    }
    this.#strategy.setDocument(info);
  }

  update(): void {
    this.#strategy.update();
  }

  panBy(deltaX: number, deltaY: number): void {
    this.#strategy.panBy(deltaX, deltaY);
  }

  goToPage(pageIndex: number): void {
    this.#strategy.goToPage(pageIndex);
  }

  fitWidth(): number {
    return this.#strategy.fitWidth();
  }

  fitPage(): number {
    return this.#strategy.fitPage();
  }

  destroy(): void {
    this.#strategy.destroy();
  }
}

interface PageSlot {
  readonly root: HTMLDivElement;
  readonly canvas: HTMLCanvasElement;
  readonly highlightLayer: HTMLDivElement;
  readonly textLayer: HTMLDivElement;
  controller?: AbortController;
  generation: number;
  renderKey?: string;
}

interface PointerPosition {
  readonly x: number;
  readonly y: number;
}

interface PageMetrics {
  readonly widths: readonly number[];
  readonly heights: readonly number[];
  readonly offsets: readonly number[];
  readonly totalHeight: number;
  readonly maxWidth: number;
}

export class ViewerViewport {
  readonly #host: ViewportHost;
  readonly #container: HTMLElement;
  readonly #root: HTMLDivElement;
  readonly #spacer: HTMLDivElement;
  readonly #slots = new Map<number, PageSlot>();
  readonly #overscan: number;
  readonly #layout: "continuous" | "single";
  readonly #onScroll = (): void => this.#handleScroll();
  readonly #onWheel = (event: WheelEvent): void => this.#handleWheel(event);
  readonly #onPointerDown = (event: PointerEvent): void =>
    this.#handlePointerDown(event);
  readonly #onPointerMove = (event: PointerEvent): void =>
    this.#handlePointerMove(event);
  readonly #onPointerUp = (event: PointerEvent): void =>
    this.#handlePointerUp(event);
  readonly #onKeyDown = (event: KeyboardEvent): void =>
    this.#handleKeyDown(event);
  readonly #onSelectionChange = (): void => this.#handleSelectionChange();
  readonly #pointers = new Map<number, PointerPosition>();
  #info: DocumentInfo | undefined;
  #frame = 0;
  #resizeObserver: ResizeObserver | undefined;
  #destroyed = false;
  #appliedZoom: number;
  #zoomSettleTimer: ReturnType<typeof setTimeout> | undefined;
  #zoomGestureActive = false;
  #lastPanPointer: PointerPosition | undefined;
  #pinchStart: { readonly distance: number; readonly zoom: number } | undefined;
  #cellAnchor:
    | {
        readonly sheetIndex: number;
        readonly row: number;
        readonly column: number;
      }
    | undefined;
  #cellFocus:
    | {
        readonly sheetIndex: number;
        readonly row: number;
        readonly column: number;
      }
    | undefined;
  #cellPointerId: number | undefined;

  constructor(
    container: HTMLElement,
    host: ViewportHost,
    options: {
      readonly overscan?: number;
      readonly layout?: "continuous" | "single";
    },
  ) {
    this.#container = container;
    this.#host = host;
    this.#overscan = Math.min(
      5,
      Math.max(0, Math.trunc(options.overscan ?? 1)),
    );
    this.#layout = options.layout ?? "continuous";
    this.#appliedZoom = host.state.zoom;
    this.#root = document.createElement("div");
    this.#root.dataset.docsViewer = "viewport";
    this.#root.tabIndex = 0;
    this.#root.setAttribute("role", "application");
    this.#root.setAttribute("aria-label", "Document viewport");
    Object.assign(this.#root.style, {
      position: "relative",
      overflow: "auto",
      width: "100%",
      height: "100%",
      minHeight: "160px",
      background: "var(--docs-viewer-background, #e9edf2)",
      touchAction: "none",
      contain: "strict",
      scrollbarGutter: "stable both-edges",
    });
    this.#spacer = document.createElement("div");
    Object.assign(this.#spacer.style, {
      position: "relative",
      minWidth: `${BASE_WIDTH}px`,
    });
    this.#root.append(this.#spacer);
    this.#container.append(this.#root);
    this.#root.addEventListener("scroll", this.#onScroll, { passive: true });
    this.#root.addEventListener("wheel", this.#onWheel, { passive: false });
    this.#root.addEventListener("pointerdown", this.#onPointerDown);
    this.#root.addEventListener("pointermove", this.#onPointerMove);
    this.#root.addEventListener("pointerup", this.#onPointerUp);
    this.#root.addEventListener("pointercancel", this.#onPointerUp);
    this.#root.addEventListener("keydown", this.#onKeyDown);
    document.addEventListener("selectionchange", this.#onSelectionChange);
    if (typeof ResizeObserver !== "undefined") {
      this.#resizeObserver = new ResizeObserver(() => this.schedule());
      this.#resizeObserver.observe(this.#root);
    }
  }

  setDocument(info: DocumentInfo | undefined): void {
    this.#info = info;
    this.#appliedZoom = this.#host.state.zoom;
    this.#root.scrollTo({ left: 0, top: 0 });
    this.#clearSlots();
    this.schedule();
  }

  update(): void {
    const zoom = this.#host.state.zoom;
    if (
      this.#info &&
      this.#layout === "continuous" &&
      zoom !== this.#appliedZoom
    ) {
      const oldMetrics = pageMetrics(this.#info, this.#appliedZoom);
      const newMetrics = pageMetrics(this.#info, zoom);
      const pageIndex = pageAtOffset(oldMetrics, this.#root.scrollTop);
      const oldTop = oldMetrics.offsets[pageIndex] ?? 0;
      const newTop = newMetrics.offsets[pageIndex] ?? 0;
      this.#root.scrollTop =
        newTop + ((this.#root.scrollTop - oldTop) * zoom) / this.#appliedZoom;
    }
    this.#appliedZoom = zoom;
    this.schedule();
  }

  panBy(deltaX: number, deltaY: number): void {
    this.#root.scrollLeft += deltaX;
    this.#root.scrollTop += deltaY;
    this.#host.onPan(this.#root.scrollLeft, this.#root.scrollTop);
    this.schedule();
  }

  goToPage(pageIndex: number): void {
    if (this.#layout === "continuous") {
      const metrics = pageMetrics(this.#info, this.#host.state.zoom);
      this.#root.scrollTop = metrics.offsets[Math.max(0, pageIndex)] ?? 0;
    }
    this.schedule();
  }

  fitWidth(): number {
    const size = naturalPageSize(this.#info, this.#host.state.pageIndex);
    return Math.max(
      0.1,
      Math.min(8, (this.#root.clientWidth - 24) / size.width),
    );
  }

  fitPage(): number {
    const size = naturalPageSize(this.#info, this.#host.state.pageIndex);
    return Math.max(
      0.1,
      Math.min(
        8,
        (this.#root.clientWidth - 24) / size.width,
        (this.#root.clientHeight - 24) / size.height,
      ),
    );
  }

  schedule(): void {
    if (this.#destroyed || this.#frame) return;
    this.#frame = requestAnimationFrame(() => {
      this.#frame = 0;
      this.#renderVisible();
    });
  }

  destroy(): void {
    if (this.#destroyed) return;
    this.#destroyed = true;
    if (this.#frame) cancelAnimationFrame(this.#frame);
    if (this.#zoomSettleTimer) clearTimeout(this.#zoomSettleTimer);
    this.#resizeObserver?.disconnect();
    this.#root.removeEventListener("scroll", this.#onScroll);
    this.#root.removeEventListener("wheel", this.#onWheel);
    this.#root.removeEventListener("pointerdown", this.#onPointerDown);
    this.#root.removeEventListener("pointermove", this.#onPointerMove);
    this.#root.removeEventListener("pointerup", this.#onPointerUp);
    this.#root.removeEventListener("pointercancel", this.#onPointerUp);
    this.#root.removeEventListener("keydown", this.#onKeyDown);
    document.removeEventListener("selectionchange", this.#onSelectionChange);
    this.#clearSlots();
    this.#root.remove();
  }

  #renderVisible(): void {
    const info = this.#info;
    const state = this.#host.state;
    if (!info || state.status !== "ready") {
      this.#spacer.style.height = "0px";
      this.#clearSlots();
      return;
    }
    const count = info.pageCount;
    const metrics = pageMetrics(info, state.zoom);
    this.#spacer.style.height =
      this.#layout === "single" ? "100%" : `${metrics.totalHeight}px`;
    this.#spacer.style.minWidth = `${metrics.maxWidth + 24}px`;
    const range =
      this.#layout === "single"
        ? { start: state.pageIndex, end: state.pageIndex + 1 }
        : visiblePageRange(
            this.#root.scrollTop,
            this.#root.clientHeight,
            metrics,
            this.#overscan,
          );
    for (const [pageIndex, slot] of this.#slots)
      if (pageIndex < range.start || pageIndex >= range.end) {
        slot.controller?.abort();
        slot.root.remove();
        this.#slots.delete(pageIndex);
      }
    for (let pageIndex = range.start; pageIndex < range.end; pageIndex += 1) {
      const slot = this.#slots.get(pageIndex) ?? this.#createSlot(pageIndex);
      const width = metrics.widths[pageIndex] ?? BASE_WIDTH * state.zoom;
      const height = metrics.heights[pageIndex] ?? BASE_HEIGHT * state.zoom;
      const top =
        this.#layout === "single" ? 12 : (metrics.offsets[pageIndex] ?? 0) + 12;
      const contentWidth = Math.max(
        this.#root.clientWidth,
        metrics.maxWidth + 24,
      );
      slot.root.style.top = `${top}px`;
      slot.root.style.left = `${Math.max(12, (contentWidth - width) / 2)}px`;
      slot.root.style.width = `${width}px`;
      slot.root.style.minHeight = `${height}px`;
      slot.root.style.transform = "none";
      const highlights = this.#host
        .getSearchMatches(pageIndex)
        .map((match) => `${match.start}:${match.end}`)
        .join(",");
      const renderKey = `${state.zoom}:${window.devicePixelRatio || 1}:${highlights}`;
      if (
        slot.renderKey === undefined ||
        (!this.#zoomGestureActive && slot.renderKey !== renderKey)
      ) {
        slot.renderKey = renderKey;
        this.#renderSlot(pageIndex, slot);
      }
    }
  }

  #createSlot(pageIndex: number): PageSlot {
    const root = document.createElement("div");
    Object.assign(root.style, {
      position: "absolute",
      background: "white",
      boxShadow: "0 2px 8px rgb(16 24 40 / 18%)",
      overflow: "hidden",
    });
    root.dataset.pageIndex = String(pageIndex);
    const canvas = document.createElement("canvas");
    Object.assign(canvas.style, {
      display: "block",
      width: "100%",
      height: "auto",
      maxWidth: "none",
    });
    const textLayer = document.createElement("div");
    const highlightLayer = document.createElement("div");
    textLayer.dataset.docsViewerLayer = "text";
    highlightLayer.dataset.docsViewerLayer = "highlight";
    for (const layer of [highlightLayer, textLayer])
      Object.assign(layer.style, {
        position: "absolute",
        inset: "0",
        overflow: "hidden",
      });
    Object.assign(highlightLayer.style, {
      pointerEvents: "none",
      userSelect: "none",
    });
    Object.assign(textLayer.style, {
      userSelect: "text",
      cursor: "text",
    });
    root.append(canvas, highlightLayer, textLayer);
    this.#spacer.append(root);
    const slot = { root, canvas, highlightLayer, textLayer, generation: 0 };
    this.#slots.set(pageIndex, slot);
    return slot;
  }

  async #renderSlot(pageIndex: number, slot: PageSlot): Promise<void> {
    slot.controller?.abort();
    const controller = new AbortController();
    slot.controller = controller;
    const generation = ++slot.generation;
    const zoom = this.#host.state.zoom;
    try {
      let renderError: unknown;
      const rendering = this.#host
        .renderPage(pageIndex, slot.canvas, {
          zoom,
          devicePixelRatio: window.devicePixelRatio || 1,
          priority:
            pageIndex === this.#host.state.pageIndex ? "visible" : "adjacent",
          signal: controller.signal,
        })
        .catch((error: unknown) => {
          renderError = error;
        });
      const runs = await this.#host.getTextRuns(pageIndex, controller.signal);
      if (controller.signal.aborted || slot.generation !== generation) return;
      await this.#buildTextLayers(
        slot.textLayer,
        slot.highlightLayer,
        pageIndex,
        runs,
        zoom,
        controller.signal,
      );
      if (controller.signal.aborted || slot.generation !== generation) return;
      await rendering;
      if (renderError) throw renderError;
      if (controller.signal.aborted || slot.generation !== generation) return;
      const cssWidth = slot.canvas.width / (window.devicePixelRatio || 1);
      const cssHeight = slot.canvas.height / (window.devicePixelRatio || 1);
      slot.root.style.width = `${cssWidth}px`;
      slot.root.style.height = `${cssHeight}px`;
    } catch (error) {
      if (!controller.signal.aborted) {
        slot.root.dataset.renderError =
          error instanceof Error ? error.message : String(error);
      }
    }
  }

  async #buildTextLayers(
    textLayer: HTMLDivElement,
    highlightLayer: HTMLDivElement,
    pageIndex: number,
    runs: readonly TextRun[],
    zoom: number,
    signal: AbortSignal,
  ): Promise<void> {
    textLayer.replaceChildren();
    highlightLayer.replaceChildren();
    const matches = this.#host.getSearchMatches(pageIndex);
    if (runs.length > 0 && runs.every(isDocxTextRun)) {
      const coordinateWidth = runs[0]!.coordinateWidth;
      const coordinateHeight = runs[0]!.coordinateHeight;
      scaleOverlay(textLayer, coordinateWidth, coordinateHeight, zoom);
      scaleOverlay(highlightLayer, coordinateWidth, coordinateHeight, zoom);
      const { buildDocxHighlightLayer, buildDocxTextLayer } =
        await import("@silurus/ooxml/docx");
      if (signal.aborted) return;
      const docxRuns = runs.map(toDocxTextRunInfo);
      const measureForFont = textMeasureFactory();
      buildDocxHighlightLayer(
        highlightLayer,
        docxRuns,
        toDocxHighlightMatches(runs, matches),
        coordinateWidth,
        coordinateHeight,
        measureForFont,
        { match: "rgb(255 215 0 / 45%)" },
      );
      buildDocxTextLayer(
        textLayer,
        docxRuns,
        coordinateWidth,
        coordinateHeight,
        undefined,
        measureForFont,
      );
      annotateTextSpans(textLayer, pageIndex, runs);
      return;
    }

    if (runs.length > 0 && runs.every(isPdfTextRun)) {
      const coordinateWidth = runs[0]!.coordinateWidth;
      const coordinateHeight = runs[0]!.coordinateHeight;
      scaleOverlay(textLayer, coordinateWidth, coordinateHeight, zoom);
      scaleOverlay(highlightLayer, coordinateWidth, coordinateHeight, zoom);
      buildPdfTextLayers(textLayer, highlightLayer, pageIndex, runs, matches);
      return;
    }

    resetOverlay(textLayer);
    resetOverlay(highlightLayer);
    let logicalOffset = 0;
    for (const run of runs) {
      const logicalStart = run.logicalStart ?? logicalOffset;
      const logicalEnd = run.logicalEnd ?? logicalStart + run.text.length;
      const span = document.createElement("span");
      span.textContent = run.text;
      span.dataset.pageIndex = String(pageIndex);
      span.dataset.start = String(logicalStart);
      span.dataset.end = String(logicalEnd);
      if (run.row !== undefined) span.dataset.row = String(run.row);
      if (run.column !== undefined) span.dataset.column = String(run.column);
      Object.assign(span.style, {
        position: "absolute",
        left: `${run.x * zoom}px`,
        top: `${run.y * zoom}px`,
        width: `${Math.max(1, run.width * zoom)}px`,
        height: `${Math.max(1, run.height * zoom)}px`,
        color: "transparent",
        whiteSpace: "pre",
        lineHeight: `${Math.max(1, run.height * zoom)}px`,
        direction: run.direction ?? "ltr",
        font: run.font,
        fontSize:
          run.font === undefined && run.fontSize !== undefined
            ? `${run.fontSize * zoom}px`
            : undefined,
        letterSpacing:
          run.letterSpacingPx === undefined
            ? undefined
            : `${run.letterSpacingPx * zoom}px`,
        transform: run.transform,
        transformOrigin: run.transform ? "top left" : undefined,
      });
      textLayer.append(span);
      if (
        matches.some(
          (match) => match.start < logicalEnd && match.end > logicalStart,
        )
      ) {
        const highlight = document.createElement("div");
        Object.assign(highlight.style, {
          position: "absolute",
          left: `${run.x * zoom}px`,
          top: `${run.y * zoom}px`,
          width: `${Math.max(1, run.width * zoom)}px`,
          height: `${Math.max(1, run.height * zoom)}px`,
          background: "var(--docs-viewer-highlight, rgb(255 215 0 / 45%))",
          pointerEvents: "none",
        });
        highlightLayer.append(highlight);
      }
      logicalOffset = logicalEnd;
    }
  }

  #handleScroll(): void {
    const state = this.#host.state;
    if (this.#layout === "continuous" && state.pageCount > 0) {
      const metrics = pageMetrics(this.#info, state.zoom);
      const visible = pageAtOffset(
        metrics,
        this.#root.scrollTop + this.#root.clientHeight * 0.35,
      );
      if (visible !== state.pageIndex) this.#host.onVisiblePage(visible);
    }
    this.#host.onPan(this.#root.scrollLeft, this.#root.scrollTop);
    this.schedule();
  }

  #handleWheel(event: WheelEvent): void {
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    const factor = Math.exp(-event.deltaY * 0.002);
    this.#deferCrispRender();
    this.#host.onZoom(this.#host.state.zoom * factor);
  }

  #handlePointerDown(event: PointerEvent): void {
    const target = event.target as HTMLElement;
    const row = Number(target.dataset.row);
    const column = Number(target.dataset.column);
    if (
      Number.isInteger(row) &&
      row > 0 &&
      Number.isInteger(column) &&
      column > 0
    ) {
      this.#root.focus({ preventScroll: true });
      const cell = { sheetIndex: this.#host.state.pageIndex, row, column };
      this.#cellAnchor = cell;
      this.#cellFocus = cell;
      this.#cellPointerId = event.pointerId;
      this.#selectCellRange(cell);
      this.#capturePointer(event);
      return;
    }
    if (event.button !== 0 || target.closest("[data-start]")) return;
    event.preventDefault();
    this.#cellAnchor = undefined;
    this.#cellFocus = undefined;
    this.#root.focus({ preventScroll: true });
    this.#pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    this.#capturePointer(event);
    if (this.#pointers.size === 1)
      this.#lastPanPointer = { x: event.clientX, y: event.clientY };
    else if (this.#pointers.size === 2) {
      this.#pinchStart = {
        distance: pointerDistance(this.#pointers),
        zoom: this.#host.state.zoom,
      };
      this.#lastPanPointer = undefined;
    }
  }

  #handlePointerMove(event: PointerEvent): void {
    if (
      this.#cellPointerId === event.pointerId &&
      this.#cellAnchor &&
      this.#cellFocus
    ) {
      const target = document.elementFromPoint(
        event.clientX,
        event.clientY,
      ) as HTMLElement | null;
      const cell = target?.closest<HTMLElement>("[data-row][data-column]");
      const row = Number(cell?.dataset.row);
      const column = Number(cell?.dataset.column);
      if (
        Number.isInteger(row) &&
        row > 0 &&
        Number.isInteger(column) &&
        column > 0
      ) {
        this.#cellFocus = {
          sheetIndex: this.#cellAnchor.sheetIndex,
          row,
          column,
        };
        this.#selectCellRange(this.#cellFocus);
      }
      return;
    }
    if (!this.#pointers.has(event.pointerId)) return;
    this.#pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (this.#pointers.size >= 2 && this.#pinchStart) {
      event.preventDefault();
      const distance = pointerDistance(this.#pointers);
      if (this.#pinchStart.distance > 0) {
        this.#deferCrispRender();
        this.#host.onZoom(
          this.#pinchStart.zoom * (distance / this.#pinchStart.distance),
        );
      }
      return;
    }
    const previous = this.#lastPanPointer;
    if (previous)
      this.panBy(previous.x - event.clientX, previous.y - event.clientY);
    this.#lastPanPointer = { x: event.clientX, y: event.clientY };
  }

  #handlePointerUp(event: PointerEvent): void {
    if (this.#cellPointerId === event.pointerId) {
      this.#cellPointerId = undefined;
      this.#releasePointer(event.pointerId);
      return;
    }
    this.#pointers.delete(event.pointerId);
    this.#releasePointer(event.pointerId);
    if (this.#pointers.size < 2) this.#pinchStart = undefined;
    this.#lastPanPointer = this.#pointers.values().next().value as
      PointerPosition | undefined;
  }

  #handleKeyDown(event: KeyboardEvent): void {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "c") {
      event.preventDefault();
      this.#host.onCopySelection();
      return;
    }
    if (event.shiftKey && this.#cellAnchor && this.#cellFocus) {
      const delta = keyCellDelta(event.key);
      if (delta) {
        event.preventDefault();
        this.#cellFocus = {
          ...this.#cellFocus,
          row: Math.max(1, this.#cellFocus.row + delta.row),
          column: Math.max(1, this.#cellFocus.column + delta.column),
        };
        this.#selectCellRange(this.#cellFocus);
        return;
      }
    }
    const pageStep = Math.max(48, this.#root.clientHeight * 0.9);
    switch (event.key) {
      case "PageDown":
        event.preventDefault();
        if (this.#layout === "single")
          this.#host.onVisiblePage(
            Math.min(
              this.#host.state.pageCount - 1,
              this.#host.state.pageIndex + 1,
            ),
          );
        else this.panBy(0, pageStep);
        break;
      case "PageUp":
        event.preventDefault();
        if (this.#layout === "single")
          this.#host.onVisiblePage(Math.max(0, this.#host.state.pageIndex - 1));
        else this.panBy(0, -pageStep);
        break;
      case "ArrowDown":
        event.preventDefault();
        this.panBy(0, 48);
        break;
      case "ArrowUp":
        event.preventDefault();
        this.panBy(0, -48);
        break;
      case "ArrowRight":
        event.preventDefault();
        this.panBy(48, 0);
        break;
      case "ArrowLeft":
        event.preventDefault();
        this.panBy(-48, 0);
        break;
      case "+":
      case "=":
        if (event.ctrlKey || event.metaKey) {
          event.preventDefault();
          this.#host.onZoom(this.#host.state.zoom * 1.2);
        }
        break;
      case "-":
        if (event.ctrlKey || event.metaKey) {
          event.preventDefault();
          this.#host.onZoom(this.#host.state.zoom / 1.2);
        }
        break;
    }
  }

  #selectCellRange(focus: {
    readonly sheetIndex: number;
    readonly row: number;
    readonly column: number;
  }): void {
    const anchor = this.#cellAnchor ?? focus;
    this.#host.onCellSelection({
      sheetIndex: anchor.sheetIndex,
      startRow: anchor.row,
      startColumn: anchor.column,
      endRow: focus.row,
      endColumn: focus.column,
    });
  }

  #capturePointer(event: PointerEvent): void {
    try {
      this.#root.setPointerCapture(event.pointerId);
    } catch {
      // Synthetic events and older engines may not expose an active pointer.
    }
  }

  #releasePointer(pointerId: number): void {
    try {
      if (this.#root.hasPointerCapture(pointerId))
        this.#root.releasePointerCapture(pointerId);
    } catch {
      // Pointer may already have been released by the browser.
    }
  }

  #deferCrispRender(): void {
    this.#zoomGestureActive = true;
    if (this.#zoomSettleTimer) clearTimeout(this.#zoomSettleTimer);
    this.#zoomSettleTimer = setTimeout(() => {
      this.#zoomSettleTimer = undefined;
      this.#zoomGestureActive = false;
      this.schedule();
    }, 120);
  }

  #handleSelectionChange(): void {
    const selection = document.getSelection();
    if (!selection || selection.isCollapsed || selection.rangeCount === 0)
      return;
    const range = selection.getRangeAt(0);
    const start = parentTextSpan(range.startContainer);
    const end = parentTextSpan(range.endContainer);
    if (
      !start ||
      !end ||
      !this.#root.contains(start) ||
      !this.#root.contains(end)
    )
      return;
    const startPageIndex = Number(start.dataset.pageIndex);
    const endPageIndex = Number(end.dataset.pageIndex);
    const startText = start.textContent ?? "";
    const endText = end.textContent ?? "";
    const rawStartOffset = endpointOffset(
      start,
      range.startContainer,
      range.startOffset,
    );
    const rawEndOffset = endpointOffset(
      end,
      range.endContainer,
      range.endOffset,
    );
    const snappedStartOffset = snapGraphemeOffset(
      startText,
      rawStartOffset,
      "start",
    );
    const snappedEndOffset = snapGraphemeOffset(endText, rawEndOffset, "end");
    if (
      rawStartOffset !== snappedStartOffset ||
      rawEndOffset !== snappedEndOffset
    ) {
      const startNode = firstTextNode(start);
      const endNode = firstTextNode(end);
      if (startNode && endNode) {
        const adjusted = document.createRange();
        adjusted.setStart(startNode, snappedStartOffset);
        adjusted.setEnd(endNode, snappedEndOffset);
        selection.removeAllRanges();
        selection.addRange(adjusted);
      }
    }
    const startOffset = Number(start.dataset.start) + snappedStartOffset;
    const endOffset = Number(end.dataset.start) + snappedEndOffset;
    this.#host.onTextSelection({
      startPageIndex,
      startOffset,
      endPageIndex,
      endOffset,
    });
  }

  #clearSlots(): void {
    for (const slot of this.#slots.values()) {
      slot.controller?.abort();
      slot.root.remove();
    }
    this.#slots.clear();
  }
}

function naturalPageSize(
  info: DocumentInfo | undefined,
  pageIndex: number,
): { readonly width: number; readonly height: number } {
  const candidate = info?.pageSizes?.[pageIndex];
  return candidate &&
    Number.isFinite(candidate.width) &&
    candidate.width > 0 &&
    Number.isFinite(candidate.height) &&
    candidate.height > 0
    ? candidate
    : { width: BASE_WIDTH, height: BASE_HEIGHT };
}

function pageMetrics(
  info: DocumentInfo | undefined,
  zoom: number,
): PageMetrics {
  const count = info?.pageCount ?? 0;
  const widths: number[] = [];
  const heights: number[] = [];
  const offsets: number[] = [];
  let top = 0;
  let maxWidth = 1;
  for (let pageIndex = 0; pageIndex < count; pageIndex += 1) {
    const size = naturalPageSize(info, pageIndex);
    const width = size.width * zoom;
    const height = size.height * zoom;
    offsets.push(top);
    widths.push(width);
    heights.push(height);
    maxWidth = Math.max(maxWidth, width);
    top += height + PAGE_GAP;
  }
  return {
    widths,
    heights,
    offsets,
    totalHeight: Math.max(1, top),
    maxWidth,
  };
}

function pageAtOffset(metrics: PageMetrics, offset: number): number {
  if (metrics.offsets.length === 0) return 0;
  const target = Math.max(0, offset);
  let low = 0;
  let high = metrics.offsets.length - 1;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (metrics.offsets[middle]! <= target) low = middle;
    else high = middle - 1;
  }
  return low;
}

function visiblePageRange(
  scrollOffset: number,
  viewportHeight: number,
  metrics: PageMetrics,
  overscan: number,
): { readonly start: number; readonly end: number } {
  const count = metrics.offsets.length;
  if (count === 0) return { start: 0, end: 0 };
  const start = pageAtOffset(metrics, scrollOffset);
  const end = pageAtOffset(
    metrics,
    Math.max(0, scrollOffset) + Math.max(0, viewportHeight),
  );
  return {
    start: Math.max(0, start - overscan),
    end: Math.min(count, end + overscan + 1),
  };
}

function pointerDistance(
  pointers: ReadonlyMap<number, PointerPosition>,
): number {
  const [first, second] = Array.from(pointers.values());
  return first && second
    ? Math.hypot(second.x - first.x, second.y - first.y)
    : 0;
}

function keyCellDelta(
  key: string,
): { readonly row: number; readonly column: number } | undefined {
  if (key === "ArrowDown") return { row: 1, column: 0 };
  if (key === "ArrowUp") return { row: -1, column: 0 };
  if (key === "ArrowRight") return { row: 0, column: 1 };
  if (key === "ArrowLeft") return { row: 0, column: -1 };
  return undefined;
}

function parentTextSpan(node: Node): HTMLElement | null {
  return node instanceof HTMLElement
    ? node.closest<HTMLElement>("[data-start]")
    : (node.parentElement?.closest<HTMLElement>("[data-start]") ?? null);
}

function endpointOffset(span: HTMLElement, node: Node, offset: number): number {
  if (node === span) return offset <= 0 ? 0 : (span.textContent?.length ?? 0);
  return offset;
}

function firstTextNode(span: HTMLElement): Text | null {
  return span.firstChild instanceof Text ? span.firstChild : null;
}

function isDocxTextRun(run: TextRun): run is TextRun & {
  readonly textLayer: "docx";
  readonly font: string;
  readonly fontSize: number;
  readonly coordinateWidth: number;
  readonly coordinateHeight: number;
} {
  return (
    run.textLayer === "docx" &&
    typeof run.font === "string" &&
    typeof run.fontSize === "number" &&
    Number.isFinite(run.fontSize) &&
    typeof run.coordinateWidth === "number" &&
    Number.isFinite(run.coordinateWidth) &&
    run.coordinateWidth > 0 &&
    typeof run.coordinateHeight === "number" &&
    Number.isFinite(run.coordinateHeight) &&
    run.coordinateHeight > 0
  );
}

function isPdfTextRun(run: TextRun): run is TextRun & {
  readonly textLayer: "pdf";
  readonly font: string;
  readonly fontSize: number;
  readonly coordinateWidth: number;
  readonly coordinateHeight: number;
} {
  return (
    run.textLayer === "pdf" &&
    typeof run.font === "string" &&
    typeof run.fontSize === "number" &&
    Number.isFinite(run.fontSize) &&
    typeof run.coordinateWidth === "number" &&
    Number.isFinite(run.coordinateWidth) &&
    run.coordinateWidth > 0 &&
    typeof run.coordinateHeight === "number" &&
    Number.isFinite(run.coordinateHeight) &&
    run.coordinateHeight > 0
  );
}

function buildPdfTextLayers(
  textLayer: HTMLDivElement,
  highlightLayer: HTMLDivElement,
  pageIndex: number,
  runs: readonly (TextRun & {
    readonly font: string;
    readonly fontSize: number;
  })[],
  matches: readonly SearchMatch[],
): void {
  const measureForFont = textMeasureFactory();
  for (const run of runs) {
    const start = run.logicalStart ?? 0;
    const end = run.logicalEnd ?? start + run.text.length;
    const measured = measureForFont(run.font)(run.text);
    const scaleX = measured > 0 ? run.width / measured : 1;
    const span = document.createElement("span");
    span.textContent = run.text;
    span.dataset.pageIndex = String(pageIndex);
    span.dataset.start = String(start);
    span.dataset.end = String(end);
    Object.assign(span.style, {
      position: "absolute",
      left: `${run.x}px`,
      top: `${run.y}px`,
      color: "transparent",
      whiteSpace: "pre",
      lineHeight: "1",
      font: run.font,
      direction: run.direction ?? "ltr",
      transform: `${run.transform ?? ""} scaleX(${scaleX})`.trim(),
      transformOrigin: "top left",
    });
    textLayer.append(span);
    if (matches.some((match) => match.start < end && match.end > start)) {
      const highlight = document.createElement("div");
      Object.assign(highlight.style, {
        position: "absolute",
        left: `${run.x}px`,
        top: `${run.y}px`,
        width: `${Math.max(1, run.width)}px`,
        height: `${Math.max(1, run.height)}px`,
        background: "var(--docs-viewer-highlight, rgb(255 215 0 / 45%))",
        pointerEvents: "none",
        transform: run.transform,
        transformOrigin: run.transform ? "top left" : undefined,
      });
      highlightLayer.append(highlight);
    }
  }
}

function toDocxTextRunInfo(
  run: TextRun & {
    readonly font: string;
    readonly fontSize: number;
  },
): DocxTextRunInfo {
  return {
    text: run.text,
    x: run.x,
    y: run.y,
    w: run.width,
    h: run.height,
    font: run.font,
    fontSize: run.fontSize,
    ...(run.letterSpacingPx === undefined
      ? {}
      : { letterSpacingPx: run.letterSpacingPx }),
    ...(run.transform === undefined ? {} : { transform: run.transform }),
    ...(run.eastAsianVert === undefined
      ? {}
      : { eastAsianVert: run.eastAsianVert }),
    ...(run.hyperlink === undefined ? {} : { hyperlink: run.hyperlink }),
  };
}

function toDocxHighlightMatches(
  runs: readonly TextRun[],
  matches: readonly SearchMatch[],
): DocxHighlightMatch[] {
  return matches.flatMap((match) => {
    const slices: { runIndex: number; start: number; end: number }[] = [];
    let offset = 0;
    runs.forEach((run, runIndex) => {
      const runStart = run.logicalStart ?? offset;
      const runEnd = run.logicalEnd ?? runStart + run.text.length;
      offset = runEnd;
      const start = Math.max(match.start, runStart);
      const end = Math.min(match.end, runEnd);
      if (start < end)
        slices.push({
          runIndex,
          start: start - runStart,
          end: end - runStart,
        });
    });
    return slices.length > 0 ? [{ slices, active: false }] : [];
  });
}

function annotateTextSpans(
  layer: HTMLDivElement,
  pageIndex: number,
  runs: readonly TextRun[],
): void {
  const spans = Array.from(layer.children).filter(
    (node): node is HTMLElement => node instanceof HTMLElement,
  );
  let offset = 0;
  runs.forEach((run, index) => {
    const span = spans[index];
    if (!span) return;
    const start = run.logicalStart ?? offset;
    const end = run.logicalEnd ?? start + run.text.length;
    offset = end;
    span.dataset.pageIndex = String(pageIndex);
    span.dataset.start = String(start);
    span.dataset.end = String(end);
    if (run.row !== undefined) span.dataset.row = String(run.row);
    if (run.column !== undefined) span.dataset.column = String(run.column);
  });
}

function scaleOverlay(
  layer: HTMLDivElement,
  width: number,
  height: number,
  zoom: number,
): void {
  Object.assign(layer.style, {
    inset: "auto",
    left: "0",
    top: "0",
    width: `${width}px`,
    height: `${height}px`,
    transform: `scale(${zoom})`,
    transformOrigin: "top left",
  });
}

function resetOverlay(layer: HTMLDivElement): void {
  Object.assign(layer.style, {
    inset: "0",
    width: "auto",
    height: "auto",
    transform: "none",
    transformOrigin: "top left",
  });
}

function textMeasureFactory(): (font: string) => (text: string) => number {
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d");
  return (font) => {
    if (context) context.font = font;
    return (text) => context?.measureText(text).width ?? 0;
  };
}
