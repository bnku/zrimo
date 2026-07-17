import type {
  DocumentInfo,
  SpreadsheetSheetInfo,
  SpreadsheetViewportRange,
} from "./contracts.js";
import type { ViewportHost } from "./viewport.js";

const DEFAULT_COLUMN_WIDTH = 64;
const DEFAULT_ROW_HEIGHT = 20;
const DEFAULT_ROW_HEADER_WIDTH = 50;
const DEFAULT_COLUMN_HEADER_HEIGHT = 22;
const OVERSCAN = 2;
const TRAILING_COLUMNS = 2;
const TRAILING_ROWS = 2;

interface PointerPosition {
  readonly x: number;
  readonly y: number;
}

interface CellAddress {
  readonly row: number;
  readonly column: number;
}

export class AxisGeometry {
  readonly #count: number;
  readonly #defaultSize: number;
  readonly #indices: readonly number[];
  readonly #sizes: readonly number[];
  readonly #prefixDeltas: readonly number[];

  constructor(
    count: number,
    defaultSize: number,
    overrides: Readonly<Record<number, number>> = {},
  ) {
    this.#count = Math.max(0, Math.trunc(count));
    this.#defaultSize = nonNegative(defaultSize, 1);
    const entries = Object.entries(overrides)
      .map(
        ([rawIndex, rawSize]) => [Number(rawIndex), Number(rawSize)] as const,
      )
      .filter(
        ([index, size]) =>
          Number.isInteger(index) &&
          index >= 1 &&
          index <= this.#count &&
          Number.isFinite(size) &&
          size >= 0,
      )
      .sort(([left], [right]) => left - right);
    this.#indices = entries.map(([index]) => index);
    this.#sizes = entries.map(([, size]) => size);
    const prefix = [0];
    for (const size of this.#sizes)
      prefix.push(prefix[prefix.length - 1]! + size - this.#defaultSize);
    this.#prefixDeltas = prefix;
  }

  get count(): number {
    return this.#count;
  }

  get totalSize(): number {
    return this.offsetOf(this.#count + 1);
  }

  sizeOf(index: number): number {
    const normalized = Math.max(1, Math.min(this.#count, Math.trunc(index)));
    const position = lowerBound(this.#indices, normalized);
    return this.#indices[position] === normalized
      ? this.#sizes[position]!
      : this.#defaultSize;
  }

  /** Offset of a 1-based band; count + 1 returns the full extent. */
  offsetOf(index: number): number {
    const normalized = Math.max(
      1,
      Math.min(this.#count + 1, Math.trunc(index)),
    );
    const overridesBefore = lowerBound(this.#indices, normalized);
    return (
      (normalized - 1) * this.#defaultSize +
      this.#prefixDeltas[overridesBefore]!
    );
  }

  /** Band containing an unscaled logical offset. Hidden zero-sized bands skip. */
  indexAt(offset: number): number {
    if (this.#count <= 0) return 1;
    const target = Math.max(0, Math.min(this.totalSize, offset));
    let low = 1;
    let high = this.#count;
    while (low < high) {
      const middle = Math.ceil((low + high) / 2);
      if (this.offsetOf(middle) <= target) low = middle;
      else high = middle - 1;
    }
    while (low < this.#count && this.sizeOf(low) === 0) low += 1;
    return low;
  }
}

export class SpreadsheetViewport {
  readonly #host: ViewportHost;
  readonly #container: HTMLElement;
  readonly #root: HTMLDivElement;
  readonly #spacer: HTMLDivElement;
  readonly #canvas: HTMLCanvasElement;
  readonly #selectionLayer: HTMLDivElement;
  readonly #selectionBox: HTMLDivElement;
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
  readonly #sheetScroll = new Map<number, { left: number; top: number }>();
  #info: DocumentInfo | undefined;
  #columns = new AxisGeometry(0, DEFAULT_COLUMN_WIDTH);
  #rows = new AxisGeometry(0, DEFAULT_ROW_HEIGHT);
  #sheet: SpreadsheetSheetInfo | undefined;
  #sheetIndex = 0;
  #frame = 0;
  #generation = 0;
  #controller: AbortController | undefined;
  #resizeObserver: ResizeObserver | undefined;
  #destroyed = false;
  #renderKey: string | undefined;
  #appliedZoom: number;
  #zoomAnchor: PointerPosition | undefined;
  #panPointer: (PointerPosition & { readonly id: number }) | undefined;
  #cellPointerId: number | undefined;
  #cellAnchor: { readonly row: number; readonly column: number } | undefined;
  #cellFocus: { readonly row: number; readonly column: number } | undefined;

  constructor(container: HTMLElement, host: ViewportHost) {
    this.#container = container;
    this.#host = host;
    this.#appliedZoom = host.state.zoom;
    this.#root = document.createElement("div");
    this.#root.dataset.docsViewer = "spreadsheet-viewport";
    this.#root.tabIndex = 0;
    this.#root.setAttribute("role", "application");
    this.#root.setAttribute("aria-label", "Spreadsheet viewport");
    Object.assign(this.#root.style, {
      position: "relative",
      overflow: "auto",
      width: "100%",
      height: "100%",
      minHeight: "160px",
      background: "var(--docs-viewer-background, #e9edf2)",
      touchAction: "none",
      contain: "strict",
    });
    this.#spacer = document.createElement("div");
    Object.assign(this.#spacer.style, {
      position: "relative",
      width: "1px",
      height: "1px",
    });
    this.#canvas = document.createElement("canvas");
    this.#canvas.dataset.docsViewerLayer = "spreadsheet-canvas";
    Object.assign(this.#canvas.style, {
      position: "absolute",
      left: "0",
      top: "0",
      display: "block",
      pointerEvents: "none",
      background: "white",
    });
    this.#selectionLayer = document.createElement("div");
    this.#selectionLayer.dataset.docsViewerLayer = "cell-selection";
    Object.assign(this.#selectionLayer.style, {
      position: "sticky",
      left: "0",
      top: "0",
      width: "0",
      height: "0",
      cursor: "cell",
      userSelect: "none",
      overflow: "hidden",
    });
    this.#selectionBox = document.createElement("div");
    Object.assign(this.#selectionBox.style, {
      position: "absolute",
      display: "none",
      boxSizing: "border-box",
      border: "2px solid var(--docs-viewer-selection, #2563eb)",
      background: "rgb(37 99 235 / 10%)",
      pointerEvents: "none",
    });
    this.#selectionLayer.append(this.#canvas, this.#selectionBox);
    this.#spacer.append(this.#selectionLayer);
    this.#root.append(this.#spacer);
    this.#container.append(this.#root);
    this.#root.addEventListener("scroll", this.#onScroll, { passive: true });
    this.#root.addEventListener("wheel", this.#onWheel, { passive: false });
    this.#selectionLayer.addEventListener("pointerdown", this.#onPointerDown);
    this.#selectionLayer.addEventListener("pointermove", this.#onPointerMove);
    this.#selectionLayer.addEventListener("pointerup", this.#onPointerUp);
    this.#selectionLayer.addEventListener("pointercancel", this.#onPointerUp);
    this.#root.addEventListener("keydown", this.#onKeyDown);
    if (typeof ResizeObserver !== "undefined") {
      this.#resizeObserver = new ResizeObserver(() => this.schedule());
      this.#resizeObserver.observe(this.#root);
    }
  }

  setDocument(info: DocumentInfo | undefined): void {
    this.#controller?.abort();
    this.#generation += 1;
    this.#info = info?.unit === "sheet" ? info : undefined;
    this.#sheetScroll.clear();
    this.#sheetIndex = this.#host.state.pageIndex;
    this.#appliedZoom = this.#host.state.zoom;
    this.#renderKey = undefined;
    this.#setSheetGeometry();
    this.#root.scrollTo({ left: 0, top: 0 });
    this.schedule();
  }

  update(): void {
    if (!this.#info) return;
    const nextSheet = this.#host.state.pageIndex;
    if (nextSheet !== this.#sheetIndex) {
      this.#sheetScroll.set(this.#sheetIndex, {
        left: this.#root.scrollLeft,
        top: this.#root.scrollTop,
      });
      this.#sheetIndex = nextSheet;
      this.#renderKey = undefined;
      this.#setSheetGeometry();
      const saved = this.#sheetScroll.get(nextSheet);
      this.#root.scrollTo({ left: saved?.left ?? 0, top: saved?.top ?? 0 });
    }
    const zoom = this.#host.state.zoom;
    if (zoom !== this.#appliedZoom) {
      const anchor = this.#zoomAnchor ?? {
        x: this.#root.clientWidth / 2,
        y: this.#root.clientHeight / 2,
      };
      const logicalX = (this.#root.scrollLeft + anchor.x) / this.#appliedZoom;
      const logicalY = (this.#root.scrollTop + anchor.y) / this.#appliedZoom;
      this.#appliedZoom = zoom;
      this.#ensureViewportGeometry();
      this.#updateExtent();
      this.#root.scrollLeft = logicalX * zoom - anchor.x;
      this.#root.scrollTop = logicalY * zoom - anchor.y;
      this.#zoomAnchor = undefined;
      this.#renderKey = undefined;
    }
    this.schedule();
  }

  panBy(deltaX: number, deltaY: number): void {
    this.#root.scrollLeft += deltaX;
    this.#root.scrollTop += deltaY;
    this.#reportPan();
    this.schedule();
  }

  goToPage(pageIndex: number): void {
    if (pageIndex === this.#sheetIndex) return;
    this.update();
  }

  fitWidth(): number {
    const usedColumns = this.#usedColumns();
    const naturalWidth =
      this.#rowHeaderWidth() + Math.max(1, usedColumns.totalSize);
    return clampZoom((this.#root.clientWidth - 2) / naturalWidth);
  }

  fitPage(): number {
    const usedColumns = this.#usedColumns();
    const usedRows = this.#usedRows();
    const naturalWidth =
      this.#rowHeaderWidth() + Math.max(1, usedColumns.totalSize);
    const naturalHeight =
      this.#columnHeaderHeight() + Math.max(1, usedRows.totalSize);
    return clampZoom(
      Math.min(
        (this.#root.clientWidth - 2) / naturalWidth,
        (this.#root.clientHeight - 2) / naturalHeight,
      ),
    );
  }

  schedule(): void {
    if (this.#destroyed || this.#frame) return;
    this.#frame = requestAnimationFrame(() => {
      this.#frame = 0;
      void this.#renderVisible();
    });
  }

  destroy(): void {
    if (this.#destroyed) return;
    this.#destroyed = true;
    this.#controller?.abort();
    if (this.#frame) cancelAnimationFrame(this.#frame);
    this.#resizeObserver?.disconnect();
    this.#root.removeEventListener("scroll", this.#onScroll);
    this.#root.removeEventListener("wheel", this.#onWheel);
    this.#selectionLayer.removeEventListener(
      "pointerdown",
      this.#onPointerDown,
    );
    this.#selectionLayer.removeEventListener(
      "pointermove",
      this.#onPointerMove,
    );
    this.#selectionLayer.removeEventListener("pointerup", this.#onPointerUp);
    this.#selectionLayer.removeEventListener(
      "pointercancel",
      this.#onPointerUp,
    );
    this.#root.removeEventListener("keydown", this.#onKeyDown);
    this.#root.remove();
  }

  async #renderVisible(): Promise<void> {
    const info = this.#info;
    const sheet = this.#sheet;
    if (
      !info ||
      !sheet ||
      this.#host.state.status !== "ready" ||
      this.#root.clientWidth <= 0 ||
      this.#root.clientHeight <= 0
    )
      return;
    this.#ensureViewportGeometry();
    this.#updateExtent();
    const render = this.#visibleRange();
    const width = this.#root.clientWidth;
    const height = this.#root.clientHeight;
    const zoom = this.#host.state.zoom;
    const dpr = window.devicePixelRatio || 1;
    const key = [
      this.#sheetIndex,
      render.range.row,
      render.range.column,
      render.range.rowCount,
      render.range.columnCount,
      render.offsetX.toFixed(3),
      render.offsetY.toFixed(3),
      width,
      height,
      zoom,
      dpr,
    ].join(":");
    this.#sizeLayers(width, height);
    this.#paintSelection();
    if (key === this.#renderKey) return;
    this.#renderKey = key;
    this.#controller?.abort();
    const controller = new AbortController();
    this.#controller = controller;
    const generation = ++this.#generation;
    const frame = document.createElement("canvas");
    try {
      await this.#host.renderSheetViewport(
        this.#sheetIndex,
        frame,
        render.range,
        {
          width,
          height,
          zoom,
          devicePixelRatio: dpr,
          scrollOffsetX: render.offsetX,
          scrollOffsetY: render.offsetY,
          priority: "visible",
          signal: controller.signal,
        },
      );
      if (
        controller.signal.aborted ||
        generation !== this.#generation ||
        this.#destroyed
      )
        return;
      this.#canvas.width = frame.width;
      this.#canvas.height = frame.height;
      const context = this.#canvas.getContext("2d");
      context?.clearRect(0, 0, frame.width, frame.height);
      context?.drawImage(frame, 0, 0);
      delete this.#root.dataset.renderError;
    } catch (error) {
      if (!controller.signal.aborted && generation === this.#generation)
        this.#root.dataset.renderError =
          error instanceof Error ? error.message : String(error);
    }
  }

  #visibleRange(): {
    readonly range: SpreadsheetViewportRange;
    readonly offsetX: number;
    readonly offsetY: number;
  } {
    const zoom = this.#host.state.zoom;
    const freezeColumns = Math.min(
      this.#columns.count,
      Math.max(0, this.#sheet?.frozenColumns ?? 0),
    );
    const freezeRows = Math.min(
      this.#rows.count,
      Math.max(0, this.#sheet?.frozenRows ?? 0),
    );
    const frozenWidth = this.#columns.offsetOf(freezeColumns + 1);
    const frozenHeight = this.#rows.offsetOf(freezeRows + 1);
    const logicalLeft = this.#root.scrollLeft / zoom;
    const logicalTop = this.#root.scrollTop / zoom;
    const startColumn = Math.max(
      freezeColumns + 1,
      this.#columns.indexAt(logicalLeft + frozenWidth),
    );
    const startRow = Math.max(
      freezeRows + 1,
      this.#rows.indexAt(logicalTop + frozenHeight),
    );
    const offsetX = Math.max(
      0,
      logicalLeft + frozenWidth - this.#columns.offsetOf(startColumn),
    );
    const offsetY = Math.max(
      0,
      logicalTop + frozenHeight - this.#rows.offsetOf(startRow),
    );
    const naturalWidth =
      this.#root.clientWidth / zoom - this.#rowHeaderWidth() - frozenWidth;
    const naturalHeight =
      this.#root.clientHeight / zoom -
      this.#columnHeaderHeight() -
      frozenHeight;
    const endColumn = this.#columns.indexAt(
      this.#columns.offsetOf(startColumn) + offsetX + Math.max(0, naturalWidth),
    );
    const endRow = this.#rows.indexAt(
      this.#rows.offsetOf(startRow) + offsetY + Math.max(0, naturalHeight),
    );
    return {
      range: {
        row: startRow,
        column: startColumn,
        rowCount: Math.max(
          1,
          Math.min(
            this.#rows.count - startRow + 1,
            endRow - startRow + 1 + OVERSCAN,
          ),
        ),
        columnCount: Math.max(
          1,
          Math.min(
            this.#columns.count - startColumn + 1,
            endColumn - startColumn + 1 + OVERSCAN,
          ),
        ),
      },
      offsetX,
      offsetY,
    };
  }

  #setSheetGeometry(): void {
    this.#sheet = this.#info?.sheets?.[this.#sheetIndex];
    this.#ensureViewportGeometry();
    this.#updateExtent();
  }

  #ensureViewportGeometry(): void {
    const sheet = this.#sheet;
    const zoom = Math.max(0.1, this.#host.state.zoom);
    const defaultColumnWidth =
      sheet?.defaultColumnWidth ?? DEFAULT_COLUMN_WIDTH;
    const defaultRowHeight = sheet?.defaultRowHeight ?? DEFAULT_ROW_HEIGHT;
    const viewportColumns = Math.ceil(
      Math.max(0, this.#root.clientWidth / zoom - this.#rowHeaderWidth()) /
        Math.max(1, defaultColumnWidth),
    );
    const viewportRows = Math.ceil(
      Math.max(0, this.#root.clientHeight / zoom - this.#columnHeaderHeight()) /
        Math.max(1, defaultRowHeight),
    );
    const columnCount = Math.max(
      1,
      sheet?.maxColumn ?? 1,
      viewportColumns + TRAILING_COLUMNS,
    );
    const rowCount = Math.max(
      1,
      sheet?.maxRow ?? 1,
      viewportRows + TRAILING_ROWS,
    );
    if (this.#columns.count !== columnCount)
      this.#columns = new AxisGeometry(
        columnCount,
        defaultColumnWidth,
        sheet?.columnWidths,
      );
    if (this.#rows.count !== rowCount)
      this.#rows = new AxisGeometry(
        rowCount,
        defaultRowHeight,
        sheet?.rowHeights,
      );
  }

  #usedColumns(): AxisGeometry {
    return new AxisGeometry(
      Math.max(1, this.#sheet?.maxColumn ?? 1),
      this.#sheet?.defaultColumnWidth ?? DEFAULT_COLUMN_WIDTH,
      this.#sheet?.columnWidths,
    );
  }

  #usedRows(): AxisGeometry {
    return new AxisGeometry(
      Math.max(1, this.#sheet?.maxRow ?? 1),
      this.#sheet?.defaultRowHeight ?? DEFAULT_ROW_HEIGHT,
      this.#sheet?.rowHeights,
    );
  }

  #updateExtent(): void {
    const zoom = this.#host.state.zoom;
    this.#spacer.style.width = `${Math.max(
      1,
      (this.#rowHeaderWidth() + this.#columns.totalSize) * zoom,
    )}px`;
    this.#spacer.style.height = `${Math.max(
      1,
      (this.#columnHeaderHeight() + this.#rows.totalSize) * zoom,
    )}px`;
  }

  #sizeLayers(width: number, height: number): void {
    for (const layer of [this.#canvas, this.#selectionLayer]) {
      layer.style.width = `${width}px`;
      layer.style.height = `${height}px`;
    }
  }

  #handleScroll(): void {
    this.#reportPan();
    this.#paintSelection();
    this.schedule();
  }

  #handleWheel(event: WheelEvent): void {
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    const bounds = this.#root.getBoundingClientRect();
    this.#zoomAnchor = {
      x: event.clientX - bounds.left,
      y: event.clientY - bounds.top,
    };
    this.#host.onZoom(this.#host.state.zoom * Math.exp(-event.deltaY * 0.002));
  }

  #handlePointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    this.#root.focus({ preventScroll: true });
    const cell = this.#cellAt(event.clientX, event.clientY);
    if (cell) {
      event.preventDefault();
      this.#cellAnchor = cell;
      this.#cellFocus = cell;
      this.#cellPointerId = event.pointerId;
      this.#emitSelection();
    } else {
      this.#panPointer = {
        id: event.pointerId,
        x: event.clientX,
        y: event.clientY,
      };
    }
    try {
      this.#selectionLayer.setPointerCapture(event.pointerId);
    } catch {
      // Synthetic events may not expose an active pointer.
    }
  }

  #handlePointerMove(event: PointerEvent): void {
    if (this.#cellPointerId === event.pointerId && this.#cellAnchor) {
      const cell = this.#cellAt(event.clientX, event.clientY);
      if (cell) {
        this.#cellFocus = cell;
        this.#emitSelection();
      }
      return;
    }
    const pointer = this.#panPointer;
    if (!pointer || pointer.id !== event.pointerId) return;
    this.panBy(pointer.x - event.clientX, pointer.y - event.clientY);
    this.#panPointer = { id: pointer.id, x: event.clientX, y: event.clientY };
  }

  #handlePointerUp(event: PointerEvent): void {
    if (this.#cellPointerId === event.pointerId)
      this.#cellPointerId = undefined;
    if (this.#panPointer?.id === event.pointerId) this.#panPointer = undefined;
    try {
      if (this.#selectionLayer.hasPointerCapture(event.pointerId))
        this.#selectionLayer.releasePointerCapture(event.pointerId);
    } catch {
      // Pointer may already have been released.
    }
  }

  #handleKeyDown(event: KeyboardEvent): void {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "c") {
      event.preventDefault();
      this.#host.onCopySelection();
      return;
    }
    const delta = cellKeyDelta(event.key);
    if (delta && this.#cellFocus) {
      event.preventDefault();
      const focus = {
        row: Math.max(
          1,
          Math.min(this.#rows.count, this.#cellFocus.row + delta.row),
        ),
        column: Math.max(
          1,
          Math.min(this.#columns.count, this.#cellFocus.column + delta.column),
        ),
      };
      if (!event.shiftKey) this.#cellAnchor = focus;
      this.#cellFocus = focus;
      this.#scrollCellIntoView(focus);
      this.#emitSelection();
      return;
    }
    const step = Math.max(48, this.#root.clientHeight * 0.9);
    switch (event.key) {
      case "PageDown":
        event.preventDefault();
        this.panBy(0, step);
        break;
      case "PageUp":
        event.preventDefault();
        this.panBy(0, -step);
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
    }
  }

  #cellAt(clientX: number, clientY: number): CellAddress | undefined {
    const bounds = this.#root.getBoundingClientRect();
    const zoom = this.#host.state.zoom;
    const x = clientX - bounds.left - this.#rowHeaderWidth() * zoom;
    const y = clientY - bounds.top - this.#columnHeaderHeight() * zoom;
    if (x < 0 || y < 0) return undefined;
    const freezeColumns = Math.max(0, this.#sheet?.frozenColumns ?? 0);
    const freezeRows = Math.max(0, this.#sheet?.frozenRows ?? 0);
    const frozenWidth = this.#columns.offsetOf(freezeColumns + 1);
    const frozenHeight = this.#rows.offsetOf(freezeRows + 1);
    const logicalX =
      x / zoom < frozenWidth
        ? x / zoom
        : this.#root.scrollLeft / zoom + x / zoom;
    const logicalY =
      y / zoom < frozenHeight
        ? y / zoom
        : this.#root.scrollTop / zoom + y / zoom;
    return {
      row: this.#rows.indexAt(logicalY),
      column: this.#columns.indexAt(logicalX),
    };
  }

  #emitSelection(): void {
    const anchor = this.#cellAnchor;
    const focus = this.#cellFocus;
    if (!anchor || !focus) return;
    this.#host.onCellSelection({
      sheetIndex: this.#sheetIndex,
      startRow: anchor.row,
      startColumn: anchor.column,
      endRow: focus.row,
      endColumn: focus.column,
    });
    this.#paintSelection();
  }

  #paintSelection(): void {
    const anchor = this.#cellAnchor;
    const focus = this.#cellFocus;
    if (!anchor || !focus) {
      this.#selectionBox.style.display = "none";
      return;
    }
    const startRow = Math.min(anchor.row, focus.row);
    const endRow = Math.max(anchor.row, focus.row);
    const startColumn = Math.min(anchor.column, focus.column);
    const endColumn = Math.max(anchor.column, focus.column);
    const topLeft = this.#cellPosition(startRow, startColumn);
    const bottomRight = this.#cellPosition(endRow + 1, endColumn + 1);
    Object.assign(this.#selectionBox.style, {
      display: "block",
      left: `${topLeft.x}px`,
      top: `${topLeft.y}px`,
      width: `${Math.max(1, bottomRight.x - topLeft.x)}px`,
      height: `${Math.max(1, bottomRight.y - topLeft.y)}px`,
    });
  }

  #cellPosition(row: number, column: number): PointerPosition {
    const zoom = this.#host.state.zoom;
    const freezeColumns = Math.max(0, this.#sheet?.frozenColumns ?? 0);
    const freezeRows = Math.max(0, this.#sheet?.frozenRows ?? 0);
    const naturalX = this.#columns.offsetOf(column);
    const naturalY = this.#rows.offsetOf(row);
    return {
      x:
        this.#rowHeaderWidth() * zoom +
        (naturalX -
          (column <= freezeColumns + 1 ? 0 : this.#root.scrollLeft / zoom)) *
          zoom,
      y:
        this.#columnHeaderHeight() * zoom +
        (naturalY - (row <= freezeRows + 1 ? 0 : this.#root.scrollTop / zoom)) *
          zoom,
    };
  }

  #scrollCellIntoView(cell: {
    readonly row: number;
    readonly column: number;
  }): void {
    const zoom = this.#host.state.zoom;
    const left =
      (this.#rowHeaderWidth() + this.#columns.offsetOf(cell.column)) * zoom;
    const right = left + this.#columns.sizeOf(cell.column) * zoom;
    const top =
      (this.#columnHeaderHeight() + this.#rows.offsetOf(cell.row)) * zoom;
    const bottom = top + this.#rows.sizeOf(cell.row) * zoom;
    if (left < this.#root.scrollLeft) this.#root.scrollLeft = left;
    else if (right > this.#root.scrollLeft + this.#root.clientWidth)
      this.#root.scrollLeft = right - this.#root.clientWidth;
    if (top < this.#root.scrollTop) this.#root.scrollTop = top;
    else if (bottom > this.#root.scrollTop + this.#root.clientHeight)
      this.#root.scrollTop = bottom - this.#root.clientHeight;
    this.#reportPan();
    this.schedule();
  }

  #rowHeaderWidth(): number {
    return this.#sheet?.rowHeaderWidth ?? DEFAULT_ROW_HEADER_WIDTH;
  }

  #columnHeaderHeight(): number {
    return this.#sheet?.columnHeaderHeight ?? DEFAULT_COLUMN_HEADER_HEIGHT;
  }

  #reportPan(): void {
    this.#host.onPan(this.#root.scrollLeft, this.#root.scrollTop);
  }
}

function lowerBound(values: readonly number[], target: number): number {
  let low = 0;
  let high = values.length;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (values[middle]! < target) low = middle + 1;
    else high = middle;
  }
  return low;
}

function nonNegative(value: number, fallback: number): number {
  return Number.isFinite(value) && value >= 0 ? value : fallback;
}

function clampZoom(value: number): number {
  return Math.max(0.1, Math.min(8, Number.isFinite(value) ? value : 1));
}

function cellKeyDelta(
  key: string,
): { readonly row: number; readonly column: number } | undefined {
  switch (key) {
    case "ArrowUp":
      return { row: -1, column: 0 };
    case "ArrowDown":
      return { row: 1, column: 0 };
    case "ArrowLeft":
      return { row: 0, column: -1 };
    case "ArrowRight":
      return { row: 0, column: 1 };
    default:
      return undefined;
  }
}
