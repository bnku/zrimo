import type {
  CellRange,
  DocumentInfo,
  SearchResult,
  TextSelection,
  ViewerApi,
  ViewerState,
  ViewerTranslations,
} from "./contracts.js";
import { ViewerError } from "./errors.js";
import { viewerCss } from "./ui-styles.js";

export class BasicViewerUi {
  readonly root: HTMLDivElement;
  readonly viewportContainer: HTMLDivElement;
  readonly #viewer: ViewerApi;
  readonly #translations: ViewerTranslations;
  readonly #toolbar: HTMLDivElement;
  readonly #searchPanel: HTMLDivElement;
  readonly #searchInput: HTMLInputElement;
  readonly #searchStatus: HTMLSpanElement;
  readonly #thumbnailPanel: HTMLElement;
  readonly #sheetTabs: HTMLDivElement;
  readonly #status: HTMLDivElement;
  readonly #pageInput: HTMLInputElement;
  readonly #pageCount: HTMLSpanElement;
  readonly #zoomLabel: HTMLSpanElement;
  readonly #buttons = new Map<string, HTMLButtonElement>();
  readonly #unsubscribes: (() => void)[] = [];
  readonly #onKeyDown = (event: KeyboardEvent): void =>
    this.#handleKeyDown(event);
  readonly #onFullscreenChange = (): void => this.#syncFullscreen();
  #info: DocumentInfo | undefined;
  #search: SearchResult | null = null;
  #selection: TextSelection | CellRange | null = null;
  #thumbnailAbort: AbortController | undefined;
  #searchTimer: ReturnType<typeof setTimeout> | undefined;
  #fallbackFullscreen = false;
  #destroyed = false;

  constructor(
    container: HTMLElement,
    viewer: ViewerApi,
    translations: ViewerTranslations,
  ) {
    this.#viewer = viewer;
    this.#translations = translations;
    this.root = document.createElement("div");
    this.root.className = "docs-viewer-ui";
    this.root.dataset.docsViewerUi = "root";
    this.root.setAttribute("lang", "");
    const style = document.createElement("style");
    style.dataset.docsViewerStyles = "";
    style.textContent = viewerCss;

    this.#toolbar = document.createElement("div");
    this.#toolbar.className = "docs-viewer-ui__toolbar";
    this.#toolbar.setAttribute("role", "toolbar");
    this.#toolbar.append(
      this.#button("thumbnails", "▦", translations.thumbnails, () =>
        this.#toggleThumbnails(),
      ),
      group(
        this.#button("previous", "‹", translations.previous, () =>
          viewer.previous(),
        ),
        this.#button("next", "›", translations.next, () => viewer.next()),
      ),
    );
    const pageGroup = group();
    pageGroup.dataset.control = "page";
    this.#pageInput = document.createElement("input");
    this.#pageInput.className = "docs-viewer-ui__page-input";
    this.#pageInput.type = "number";
    this.#pageInput.min = "1";
    this.#pageInput.setAttribute("aria-label", translations.page);
    this.#pageInput.addEventListener("change", () =>
      viewer.goToPage(Number(this.#pageInput.value) - 1),
    );
    this.#pageCount = document.createElement("span");
    pageGroup.append(this.#pageInput, this.#pageCount);
    this.#toolbar.append(
      pageGroup,
      group(
        this.#button("zoom-out", "−", translations.zoomOut, () =>
          viewer.zoomOut(),
        ),
        this.#button("zoom-in", "+", translations.zoomIn, () =>
          viewer.zoomIn(),
        ),
      ),
    );
    this.#zoomLabel = document.createElement("span");
    this.#zoomLabel.dataset.control = "zoom-label";
    this.#toolbar.append(
      this.#zoomLabel,
      group(
        this.#button("fit-width", "↔", translations.fitWidth, () =>
          viewer.fitWidth(),
        ),
        this.#button("fit-page", "□", translations.fitPage, () =>
          viewer.fitPage(),
        ),
      ),
      spacer(),
      this.#button("search", "⌕", translations.search, () =>
        this.#toggleSearch(),
      ),
      this.#button(
        "fullscreen",
        "⛶",
        translations.fullscreen,
        () => void this.#toggleFullscreen(),
      ),
      this.#button("download", "⇩", translations.download, () => {
        try {
          viewer.downloadOriginal();
        } catch (error) {
          this.#showStatus(
            error instanceof Error ? error.message : String(error),
          );
        }
      }),
    );

    this.#searchPanel = document.createElement("div");
    this.#searchPanel.className = "docs-viewer-ui__search";
    this.#searchPanel.hidden = true;
    this.#searchInput = document.createElement("input");
    this.#searchInput.type = "search";
    this.#searchInput.placeholder = translations.searchPlaceholder;
    this.#searchInput.setAttribute("aria-label", translations.search);
    this.#searchInput.addEventListener("input", () => this.#scheduleSearch());
    this.#searchInput.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        if (event.shiftKey) viewer.searchPrevious();
        else viewer.searchNext();
      }
    });
    this.#searchStatus = document.createElement("span");
    this.#searchStatus.className = "docs-viewer-ui__search-status";
    this.#searchPanel.append(
      this.#searchInput,
      this.#button("search-previous", "‹", translations.previous, () =>
        viewer.searchPrevious(),
      ),
      this.#button("search-next", "›", translations.next, () =>
        viewer.searchNext(),
      ),
      this.#searchStatus,
      this.#button("search-close", "×", translations.close, () =>
        this.#closeSearch(),
      ),
    );

    const body = document.createElement("div");
    body.className = "docs-viewer-ui__body";
    this.#thumbnailPanel = document.createElement("aside");
    this.#thumbnailPanel.className = "docs-viewer-ui__panel";
    this.#thumbnailPanel.dataset.panel = "thumbnails";
    this.#thumbnailPanel.hidden = true;
    this.#thumbnailPanel.setAttribute("aria-label", translations.thumbnails);
    this.viewportContainer = document.createElement("div");
    this.viewportContainer.className = "docs-viewer-ui__viewport";
    body.append(this.#thumbnailPanel, this.viewportContainer);

    this.#sheetTabs = document.createElement("div");
    this.#sheetTabs.className = "docs-viewer-ui__sheets";
    this.#sheetTabs.hidden = true;
    this.#sheetTabs.setAttribute("aria-label", translations.sheets);
    this.#status = document.createElement("div");
    this.#status.className = "docs-viewer-ui__status";
    this.#status.hidden = true;
    this.#status.setAttribute("role", "status");
    this.root.append(
      style,
      this.#toolbar,
      this.#searchPanel,
      body,
      this.#sheetTabs,
      this.#status,
    );
    container.append(this.root);

    this.#unsubscribes.push(
      viewer.on("statechange", (state) => this.#syncState(state)),
      viewer.on("ready", () => {
        this.#info = viewer.getDocumentInfo();
        this.#renderSheetTabs();
        this.#syncState(viewer.state);
      }),
      viewer.on("progress", (progress) => {
        if (viewer.state.status === "loading")
          this.#showStatus(
            progress.ratio === undefined
              ? translations.loading
              : `${translations.loading} ${Math.round(progress.ratio * 100)}%`,
          );
      }),
      viewer.on("error", (error) => this.#showStatus(error.message)),
      viewer.on("warning", (warning) =>
        this.#showStatus(warning.message, 3500),
      ),
      viewer.on("searchchange", (result) => {
        this.#search = result;
        this.#syncSearch();
      }),
      viewer.on("selectionchange", (selection) => {
        this.#selection = selection;
        this.#renderSheetTabs();
      }),
      viewer.on("pagechange", () => {
        if (!this.#thumbnailPanel.hidden) void this.#renderThumbnails();
        this.#renderSheetTabs();
      }),
    );
    this.root.addEventListener("keydown", this.#onKeyDown);
    document.addEventListener("fullscreenchange", this.#onFullscreenChange);
    this.#syncState(viewer.state);
  }

  destroy(): void {
    if (this.#destroyed) return;
    this.#destroyed = true;
    if (this.#searchTimer) clearTimeout(this.#searchTimer);
    this.#thumbnailAbort?.abort();
    for (const unsubscribe of this.#unsubscribes) unsubscribe();
    this.root.removeEventListener("keydown", this.#onKeyDown);
    document.removeEventListener("fullscreenchange", this.#onFullscreenChange);
    if (document.fullscreenElement === this.root)
      void document.exitFullscreen();
    this.root.remove();
  }

  #button(
    id: string,
    text: string,
    label: string,
    action: () => void,
  ): HTMLButtonElement {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "docs-viewer-ui__button";
    button.dataset.action = id;
    button.textContent = text;
    button.title = label;
    button.setAttribute("aria-label", label);
    button.addEventListener("click", action);
    this.#buttons.set(id, button);
    return button;
  }

  #syncState(state: ViewerState): void {
    const ready = state.status === "ready";
    this.#pageInput.value = String(state.pageIndex + 1);
    this.#pageInput.max = String(Math.max(1, state.pageCount));
    this.#pageCount.textContent = `${this.#translations.of} ${state.pageCount}`;
    this.#zoomLabel.textContent = `${Math.round(state.zoom * 100)}%`;
    this.#buttons.get("previous")!.disabled = !ready || state.pageIndex <= 0;
    this.#buttons.get("next")!.disabled =
      !ready || state.pageIndex >= state.pageCount - 1;
    this.#buttons.get("download")!.disabled = !ready;
    this.#buttons.get("search")!.disabled =
      !ready || !this.#info?.capabilities?.search;
    const single = state.pageCount <= 1;
    const pageControls = this.#toolbar.querySelector<HTMLElement>(
      '[data-control="page"]',
    );
    if (pageControls)
      pageControls.hidden = single || this.#info?.unit === "sheet";
    this.#buttons.get("previous")!.hidden =
      single || this.#info?.unit === "sheet";
    this.#buttons.get("next")!.hidden = single || this.#info?.unit === "sheet";
    const thumbnailsAvailable = Boolean(
      ready &&
      !single &&
      this.#info?.capabilities?.thumbnails &&
      this.#info.unit !== "sheet",
    );
    this.#buttons.get("thumbnails")!.hidden = !thumbnailsAvailable;
    if (!thumbnailsAvailable) this.#closeThumbnails();
    if (state.status !== "loading" && state.status !== "error")
      this.#hideStatus();
    if (state.status === "idle") {
      this.#info = undefined;
      this.#search = null;
      this.#selection = null;
      this.#renderSheetTabs();
    }
  }

  #toggleSearch(): void {
    const open = this.#searchPanel.hidden;
    this.#searchPanel.hidden = !open;
    this.#buttons.get("search")!.setAttribute("aria-pressed", String(open));
    if (open) this.#searchInput.focus();
  }

  #closeSearch(): void {
    this.#searchPanel.hidden = true;
    this.#buttons.get("search")!.setAttribute("aria-pressed", "false");
    this.#searchInput.value = "";
    this.#viewer.clearSearch();
  }

  #scheduleSearch(): void {
    if (this.#searchTimer) clearTimeout(this.#searchTimer);
    this.#searchTimer = setTimeout(() => {
      this.#searchTimer = undefined;
      void this.#viewer
        .search(this.#searchInput.value)
        .catch((error: unknown) => {
          if (!(error instanceof ViewerError) || error.code !== "aborted")
            this.#showStatus(
              error instanceof Error ? error.message : String(error),
            );
        });
    }, 120);
  }

  #syncSearch(): void {
    const result = this.#search;
    if (!result || result.matches.length === 0) {
      this.#searchStatus.textContent = this.#searchInput.value
        ? this.#translations.noMatches
        : "";
      return;
    }
    this.#searchStatus.textContent = `${result.activeIndex + 1}/${result.matches.length}`;
  }

  #toggleThumbnails(): void {
    const open = this.#thumbnailPanel.hidden;
    this.#thumbnailPanel.hidden = !open;
    this.#buttons.get("thumbnails")!.setAttribute("aria-pressed", String(open));
    if (open) void this.#renderThumbnails();
    else this.#thumbnailAbort?.abort();
  }

  #closeThumbnails(): void {
    this.#thumbnailPanel.hidden = true;
    this.#buttons.get("thumbnails")!.setAttribute("aria-pressed", "false");
    this.#thumbnailAbort?.abort();
    this.#thumbnailPanel.replaceChildren();
  }

  async #renderThumbnails(): Promise<void> {
    if (this.#thumbnailPanel.hidden || this.#viewer.state.status !== "ready")
      return;
    this.#thumbnailAbort?.abort();
    const controller = new AbortController();
    this.#thumbnailAbort = controller;
    const state = this.#viewer.state;
    const start = Math.max(0, state.pageIndex - 5);
    const end = Math.min(state.pageCount, start + 11);
    this.#thumbnailPanel.replaceChildren();
    for (let pageIndex = start; pageIndex < end; pageIndex += 1) {
      if (controller.signal.aborted) return;
      const button = document.createElement("button");
      button.type = "button";
      button.className = "docs-viewer-ui__thumbnail";
      button.dataset.pageIndex = String(pageIndex);
      button.setAttribute(
        "aria-label",
        `${this.#translations.page} ${pageIndex + 1}`,
      );
      button.setAttribute(
        "aria-current",
        pageIndex === state.pageIndex ? "page" : "false",
      );
      const canvas = document.createElement("canvas");
      const label = document.createElement("span");
      label.textContent = String(pageIndex + 1);
      button.append(canvas, label);
      button.addEventListener("click", () => this.#viewer.goToPage(pageIndex));
      this.#thumbnailPanel.append(button);
      try {
        await this.#viewer.renderThumbnail(pageIndex, canvas, {
          maxWidth: 180,
          maxHeight: 220,
          signal: controller.signal,
        });
      } catch (error) {
        if (!(error instanceof ViewerError) || error.code !== "aborted")
          button.dataset.renderError = "true";
      }
    }
  }

  #renderSheetTabs(): void {
    const names = this.#info?.sheetNames;
    if (this.#info?.unit !== "sheet" || !names) {
      this.#sheetTabs.hidden = true;
      this.#sheetTabs.replaceChildren();
      return;
    }
    this.#sheetTabs.hidden = false;
    this.#sheetTabs.replaceChildren();
    names.forEach((name, sheetIndex) => {
      const button = this.#button(`sheet-${sheetIndex}`, name, name, () =>
        this.#viewer.setSheet(sheetIndex),
      );
      button.setAttribute(
        "aria-pressed",
        String(sheetIndex === this.#viewer.state.pageIndex),
      );
      this.#sheetTabs.append(button);
    });
    if (this.#selection && "sheetIndex" in this.#selection) {
      const range = document.createElement("span");
      range.className = "docs-viewer-ui__label";
      range.textContent = `${this.#translations.selectedRange}: R${this.#selection.startRow}C${this.#selection.startColumn}:R${this.#selection.endRow}C${this.#selection.endColumn}`;
      this.#sheetTabs.append(range);
    }
  }

  async #toggleFullscreen(): Promise<void> {
    if (document.fullscreenElement === this.root) {
      await document.exitFullscreen();
      return;
    }
    if (this.#fallbackFullscreen) {
      this.#setFallbackFullscreen(false);
      return;
    }
    try {
      if (!this.root.requestFullscreen)
        throw new Error("Fullscreen API unavailable");
      await this.root.requestFullscreen();
    } catch {
      this.#setFallbackFullscreen(true);
    }
  }

  #setFallbackFullscreen(active: boolean): void {
    this.#fallbackFullscreen = active;
    this.root.classList.toggle("docs-viewer-ui--fullscreen-fallback", active);
    this.#syncFullscreen();
  }

  #syncFullscreen(): void {
    const active =
      document.fullscreenElement === this.root || this.#fallbackFullscreen;
    const button = this.#buttons.get("fullscreen")!;
    button.setAttribute("aria-pressed", String(active));
    button.title = active
      ? this.#translations.exitFullscreen
      : this.#translations.fullscreen;
    button.setAttribute("aria-label", button.title);
  }

  #handleKeyDown(event: KeyboardEvent): void {
    if (event.defaultPrevented) return;
    const editing =
      event.target instanceof HTMLInputElement ||
      event.target instanceof HTMLTextAreaElement ||
      (event.target instanceof HTMLElement && event.target.isContentEditable);
    if (event.key === "Escape") {
      this.#searchPanel.hidden = true;
      this.#closeThumbnails();
      if (this.#fallbackFullscreen) this.#setFallbackFullscreen(false);
      return;
    }
    if (editing) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      if (this.#searchPanel.hidden) this.#toggleSearch();
      else this.#searchInput.focus();
    } else if (
      (event.ctrlKey || event.metaKey) &&
      event.key.toLowerCase() === "c"
    ) {
      event.preventDefault();
      void this.#viewer.copySelection();
    } else if (event.key === "F11") {
      event.preventDefault();
      void this.#toggleFullscreen();
    }
  }

  #showStatus(message: string, timeout = 0): void {
    this.#status.textContent = message;
    this.#status.hidden = false;
    if (timeout > 0)
      setTimeout(() => {
        if (this.#status.textContent === message) this.#hideStatus();
      }, timeout);
  }

  #hideStatus(): void {
    this.#status.hidden = true;
    this.#status.textContent = "";
  }
}

function group(...children: HTMLElement[]): HTMLSpanElement {
  const element = document.createElement("span");
  element.className = "docs-viewer-ui__group";
  element.append(...children);
  return element;
}

function spacer(): HTMLSpanElement {
  const element = document.createElement("span");
  element.className = "docs-viewer-ui__spacer";
  return element;
}
