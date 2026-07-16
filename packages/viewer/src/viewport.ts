import type {
  CellRange,
  DocumentInfo,
  HeadlessRenderOptions,
  SearchMatch,
  TextRun,
  TextSelectionRange,
  ViewerState,
} from "./contracts.js";
import { visibleRange } from "./interaction.js";

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
}

interface PageSlot {
  readonly root: HTMLDivElement;
  readonly canvas: HTMLCanvasElement;
  readonly textLayer: HTMLDivElement;
  controller?: AbortController;
  generation: number;
  renderKey?: string;
}

interface PointerPosition {
  readonly x: number;
  readonly y: number;
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
      const oldExtent = BASE_HEIGHT * this.#appliedZoom + PAGE_GAP;
      const newExtent = BASE_HEIGHT * zoom + PAGE_GAP;
      this.#root.scrollTop = (this.#root.scrollTop / oldExtent) * newExtent;
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
      const extent = BASE_HEIGHT * this.#host.state.zoom + PAGE_GAP;
      this.#root.scrollTop = Math.max(0, pageIndex) * extent;
    }
    this.schedule();
  }

  fitWidth(): number {
    return Math.max(
      0.1,
      Math.min(8, (this.#root.clientWidth - 24) / BASE_WIDTH),
    );
  }

  fitPage(): number {
    return Math.max(
      0.1,
      Math.min(
        8,
        (this.#root.clientWidth - 24) / BASE_WIDTH,
        (this.#root.clientHeight - 24) / BASE_HEIGHT,
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
    const extent = BASE_HEIGHT * state.zoom + PAGE_GAP;
    const width = BASE_WIDTH * state.zoom;
    const count = info.pageCount;
    this.#spacer.style.height =
      this.#layout === "single" ? "100%" : `${Math.max(1, count * extent)}px`;
    this.#spacer.style.minWidth = `${width + 24}px`;
    const range =
      this.#layout === "single"
        ? { start: state.pageIndex, end: state.pageIndex + 1 }
        : visibleRange(
            this.#root.scrollTop,
            this.#root.clientHeight,
            extent,
            count,
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
      const top = this.#layout === "single" ? 12 : pageIndex * extent + 12;
      slot.root.style.top = `${top}px`;
      slot.root.style.left = "50%";
      slot.root.style.width = `${width}px`;
      slot.root.style.minHeight = `${BASE_HEIGHT * state.zoom}px`;
      slot.root.style.transform = "translateX(-50%)";
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
    Object.assign(textLayer.style, {
      position: "absolute",
      inset: "0",
      overflow: "hidden",
      userSelect: "text",
      cursor: "text",
    });
    root.append(canvas, textLayer);
    this.#spacer.append(root);
    const slot = { root, canvas, textLayer, generation: 0 };
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
      this.#buildTextLayer(slot.textLayer, pageIndex, runs, zoom);
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

  #buildTextLayer(
    layer: HTMLDivElement,
    pageIndex: number,
    runs: readonly TextRun[],
    zoom: number,
  ): void {
    layer.replaceChildren();
    const matches = this.#host.getSearchMatches(pageIndex);
    let logicalOffset = 0;
    for (const run of runs) {
      const span = document.createElement("span");
      span.textContent = run.text;
      span.dataset.pageIndex = String(pageIndex);
      span.dataset.start = String(logicalOffset);
      span.dataset.end = String(logicalOffset + run.text.length);
      if (run.row !== undefined) span.dataset.row = String(run.row);
      if (run.column !== undefined) span.dataset.column = String(run.column);
      const highlighted = matches.some(
        (match) =>
          match.start < logicalOffset + run.text.length &&
          match.end > logicalOffset,
      );
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
        background: highlighted
          ? "var(--docs-viewer-highlight, rgb(255 215 0 / 45%))"
          : "transparent",
      });
      layer.append(span);
      logicalOffset += run.text.length;
    }
  }

  #handleScroll(): void {
    const state = this.#host.state;
    if (this.#layout === "continuous" && state.pageCount > 0) {
      const extent = BASE_HEIGHT * state.zoom + PAGE_GAP;
      const visible = Math.min(
        state.pageCount - 1,
        Math.max(
          0,
          Math.floor((this.#root.scrollTop + extent * 0.35) / extent),
        ),
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
    const startOffset = Number(start.dataset.start) + range.startOffset;
    const endOffset = Number(end.dataset.start) + range.endOffset;
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
