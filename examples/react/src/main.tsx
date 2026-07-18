import {
  StrictMode,
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type RefObject,
} from "react";
import { createRoot } from "react-dom/client";
import {
  ViewerClient,
  ViewerError,
  type CellRange,
  type CellSelection,
  type DocumentInfo,
  type SearchResult,
  type TextSelection,
  type ViewerApi,
  type ViewerProgress,
  type ViewerState,
} from "@zrimo/viewer";
import "@zrimo/viewer/styles.css";
import "./styles.css";

type ExampleTab = "built-in" | "react-ui" | "headless";

const INITIAL_STATE: ViewerState = {
  status: "idle",
  pageIndex: 0,
  pageCount: 0,
  zoom: 1,
  fit: "none",
  panX: 0,
  panY: 0,
};

const TABS: readonly {
  id: ExampleTab;
  label: string;
  eyebrow: string;
}[] = [
  { id: "built-in", label: "Built-in UI", eyebrow: "ui: true" },
  { id: "react-ui", label: "React controls", eyebrow: "ui: false" },
  { id: "headless", label: "Headless API", eyebrow: "no container" },
];

interface ViewerSessionOptions {
  readonly file: File | undefined;
  readonly host?: RefObject<HTMLDivElement | null>;
  readonly ui?: boolean;
  readonly layout?: "continuous" | "single";
}

interface ViewerSession {
  readonly viewer: ViewerApi | null;
  readonly state: ViewerState;
  readonly info: DocumentInfo | null;
  readonly progress: string;
  readonly error: string;
  readonly warnings: readonly string[];
  readonly events: readonly string[];
  readonly selection: TextSelection | CellRange | CellSelection | null;
  readonly searchResult: SearchResult | null;
}

function useViewerSession({
  file,
  host,
  ui = false,
  layout = "continuous",
}: ViewerSessionOptions): ViewerSession {
  const [viewer, setViewer] = useState<ViewerApi | null>(null);
  const [state, setState] = useState<ViewerState>(INITIAL_STATE);
  const [info, setInfo] = useState<DocumentInfo | null>(null);
  const [progress, setProgress] = useState("Waiting for a document");
  const [error, setError] = useState("");
  const [warnings, setWarnings] = useState<readonly string[]>([]);
  const [events, setEvents] = useState<readonly string[]>([]);
  const [selection, setSelection] = useState<
    TextSelection | CellRange | CellSelection | null
  >(null);
  const [searchResult, setSearchResult] = useState<SearchResult | null>(null);

  useEffect(() => {
    const container = host?.current;
    if (host && !container) return;

    const client = ViewerClient.create({
      assetBaseUrl: new URL(import.meta.env.BASE_URL, location.href),
    });
    const instance = client.createViewer({
      ...(container ? { container } : {}),
      ui,
      locale: "en",
      fit: "none",
      layout,
      overscan: 1,
    });
    const pushEvent = (message: string) =>
      setEvents((current) => [message, ...current].slice(0, 8));
    const unsubscribers = [
      instance.on("statechange", (next) => {
        setState(next);
        pushEvent(`statechange · ${next.status}`);
        if (next.status === "loading" || next.status === "idle") {
          setInfo(null);
          setSelection(null);
          setSearchResult(null);
        }
      }),
      instance.on("ready", (next) => {
        setInfo(instance.getDocumentInfo());
        pushEvent(`ready · ${next.format ?? "document"}`);
      }),
      instance.on("pagechange", ({ pageIndex, pageCount }) =>
        pushEvent(`pagechange · ${pageIndex + 1}/${pageCount}`),
      ),
      instance.on("zoomchange", ({ zoom, fit }) =>
        pushEvent(`zoomchange · ${Math.round(zoom * 100)}% (${fit})`),
      ),
      instance.on("progress", (next) => setProgress(formatProgress(next))),
      instance.on("warning", (warning) => {
        setWarnings((current) =>
          [
            warning.message,
            ...current.filter((item) => item !== warning.message),
          ].slice(0, 3),
        );
        pushEvent(`warning · ${warning.code}`);
      }),
      instance.on("selectionchange", (next) => {
        setSelection(next);
        pushEvent(
          next ? "selectionchange · selected" : "selectionchange · clear",
        );
      }),
      instance.on("searchchange", (next) => {
        setSearchResult(next);
        if (next) pushEvent(`searchchange · ${next.matches.length} matches`);
      }),
    ];

    setViewer(instance);
    setState(instance.state);
    return () => {
      for (const unsubscribe of unsubscribers) unsubscribe();
      void instance.destroy().finally(() => client.destroy());
    };
  }, [host, layout, ui]);

  useEffect(() => {
    if (!viewer || !file) return;
    let current = true;
    setError("");
    setWarnings([]);
    void viewer.load(file, { fileName: file.name }).catch((cause: unknown) => {
      if (current && !isAbort(cause)) setError(errorMessage(cause));
    });
    return () => {
      current = false;
    };
  }, [file, viewer]);

  return {
    viewer,
    state,
    info,
    progress,
    error,
    warnings,
    events,
    selection,
    searchResult,
  };
}

function App() {
  const [tab, setTab] = useState<ExampleTab>("built-in");
  const [file, setFile] = useState<File>();
  const selectTabFromKeyboard = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) => {
    let nextIndex: number | undefined;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % TABS.length;
    if (event.key === "ArrowLeft")
      nextIndex = (index - 1 + TABS.length) % TABS.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = TABS.length - 1;
    if (nextIndex === undefined) return;
    event.preventDefault();
    const nextTab = TABS[nextIndex]!;
    setTab(nextTab.id);
    requestAnimationFrame(() =>
      document.getElementById(`tab-${nextTab.id}`)?.focus(),
    );
  };

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            Z
          </span>
          <div>
            <strong>Zrimo</strong>
            <span>React integration playground</span>
          </div>
        </div>
        <label className="file-picker">
          <span>{file ? "Replace document" : "Open document"}</span>
          <input
            aria-label="Open a document"
            type="file"
            onClick={(event) => {
              event.currentTarget.value = "";
            }}
            onChange={(event) => setFile(event.currentTarget.files?.[0])}
          />
        </label>
        <div className="file-summary" title={file?.name}>
          <span className={file ? "file-dot file-dot--ready" : "file-dot"} />
          {file ? (
            <>
              <strong>{file.name}</strong>
              <span>{formatBytes(file.size)}</span>
            </>
          ) : (
            <span>PDF, Office, images, CSV or SVG</span>
          )}
        </div>
      </header>

      <nav className="tabs" role="tablist" aria-label="Integration examples">
        {TABS.map((item, index) => (
          <button
            key={item.id}
            id={`tab-${item.id}`}
            type="button"
            role="tab"
            aria-selected={tab === item.id}
            aria-controls={`panel-${item.id}`}
            tabIndex={tab === item.id ? 0 : -1}
            className={tab === item.id ? "tab tab--active" : "tab"}
            onClick={() => setTab(item.id)}
            onKeyDown={(event) => selectTabFromKeyboard(event, index)}
          >
            <span>{item.label}</span>
            <small>{item.eyebrow}</small>
          </button>
        ))}
      </nav>

      <main className="example-main">
        <section
          id={`panel-${tab}`}
          role="tabpanel"
          aria-labelledby={`tab-${tab}`}
          className="tab-panel"
        >
          {tab === "built-in" && <BuiltInExample file={file} />}
          {tab === "react-ui" && <ReactUiExample file={file} />}
          {tab === "headless" && <HeadlessExample file={file} />}
        </section>
      </main>
    </div>
  );
}

function BuiltInExample({ file }: { readonly file: File | undefined }) {
  const host = useRef<HTMLDivElement>(null);
  const session = useViewerSession({ file, host, ui: true });

  return (
    <div className="example-column">
      <ExampleIntro
        code={"client.createViewer({ container, ui: true })"}
        title="Ready-made viewer UI"
        description="The package mounts its localized toolbar, search, thumbnails, fullscreen and sheet tabs. React only owns the lifecycle and the file source."
        session={session}
      />
      <div className="viewer-frame viewer-frame--built-in">
        <div ref={host} className="viewer-host" />
        {!file && <EmptyDocument />}
        <LoadingOverlay file={file} session={session} />
      </div>
    </div>
  );
}

function ReactUiExample({ file }: { readonly file: File | undefined }) {
  const host = useRef<HTMLDivElement>(null);
  const session = useViewerSession({ file, host, ui: false });
  const { viewer, state, info, selection, searchResult } = session;
  const ready = state.status === "ready";
  const unitName = info?.unit === "sheet" ? "Sheet" : "Page";
  const [query, setQuery] = useState("");
  const [actionMessage, setActionMessage] = useState("");

  const run = (action: () => unknown | Promise<unknown>) => {
    setActionMessage("");
    try {
      void Promise.resolve(action()).catch((cause: unknown) =>
        setActionMessage(errorMessage(cause)),
      );
    } catch (cause) {
      setActionMessage(errorMessage(cause));
    }
  };
  const submitSearch = (event: FormEvent) => {
    event.preventDefault();
    if (!viewer || !ready) return;
    if (!query) {
      viewer.clearSearch();
      return;
    }
    run(() => viewer.search(query));
  };

  return (
    <div className="example-column">
      <ExampleIntro
        code={"client.createViewer({ container, ui: false })"}
        title="Package viewport, React interface"
        description="The built-in interface is completely hidden. Every control below is ordinary React calling the public ViewerApi and subscribing to its events."
        session={session}
      />
      <div className="custom-workspace">
        <div className="custom-main">
          <div className="custom-toolbar" aria-label="React viewer controls">
            <div className="control-group">
              <button
                type="button"
                title={`Previous ${unitName.toLowerCase()}`}
                disabled={!ready || state.pageIndex === 0}
                onClick={() => viewer?.previous()}
              >
                ←
              </button>
              <label className="page-control">
                <span>{unitName}</span>
                <input
                  aria-label={unitName}
                  type="number"
                  min={1}
                  max={Math.max(1, state.pageCount)}
                  value={state.pageCount ? state.pageIndex + 1 : 0}
                  disabled={!ready}
                  onChange={(event) =>
                    viewer?.goToPage(Number(event.currentTarget.value) - 1)
                  }
                />
                <span>/ {state.pageCount || "—"}</span>
              </label>
              <button
                type="button"
                title={`Next ${unitName.toLowerCase()}`}
                disabled={!ready || state.pageIndex >= state.pageCount - 1}
                onClick={() => viewer?.next()}
              >
                →
              </button>
            </div>

            <div className="control-group">
              <button
                type="button"
                title="Zoom out"
                disabled={!ready}
                onClick={() => viewer?.zoomOut()}
              >
                −
              </button>
              <output className="zoom-output">
                {Math.round(state.zoom * 100)}%
              </output>
              <button
                type="button"
                title="Zoom in"
                disabled={!ready}
                onClick={() => viewer?.zoomIn()}
              >
                +
              </button>
              <button
                type="button"
                className={state.fit === "width" ? "is-active" : ""}
                disabled={!ready}
                onClick={() => viewer?.fitWidth()}
              >
                Fit width
              </button>
              <button
                type="button"
                className={state.fit === "page" ? "is-active" : ""}
                disabled={!ready}
                onClick={() => viewer?.fitPage()}
              >
                Fit page
              </button>
            </div>

            <form className="search-control" onSubmit={submitSearch}>
              <input
                aria-label="Search document"
                type="search"
                value={query}
                placeholder="Search text"
                disabled={!ready || !info?.capabilities?.search}
                onChange={(event) => setQuery(event.currentTarget.value)}
              />
              <button
                type="submit"
                disabled={!ready || !info?.capabilities?.search}
              >
                Search
              </button>
              <button
                type="button"
                title="Previous match"
                disabled={!searchResult?.matches.length}
                onClick={() => viewer?.searchPrevious()}
              >
                ↑
              </button>
              <button
                type="button"
                title="Next match"
                disabled={!searchResult?.matches.length}
                onClick={() => viewer?.searchNext()}
              >
                ↓
              </button>
              <span className="matches-output">
                {searchResult?.matches.length
                  ? `${searchResult.activeIndex + 1}/${searchResult.matches.length}`
                  : ""}
              </span>
            </form>

            <div className="control-group control-group--end">
              <button
                type="button"
                disabled={!selection}
                onClick={() =>
                  run(async () => {
                    const copied = await viewer?.copySelection();
                    setActionMessage(
                      copied
                        ? `Copied ${copied.length} characters`
                        : "Nothing selected",
                    );
                  })
                }
              >
                Copy selection
              </button>
              <button
                type="button"
                disabled={!ready}
                onClick={() => run(() => viewer?.downloadOriginal(file?.name))}
              >
                Download original
              </button>
            </div>
          </div>

          {(actionMessage || session.error) && (
            <div
              className="action-message"
              role={session.error ? "alert" : "status"}
            >
              {session.error || actionMessage}
            </div>
          )}
          <div className="viewer-frame viewer-frame--custom">
            <div ref={host} className="viewer-host" />
            {!file && <EmptyDocument />}
            <LoadingOverlay file={file} session={session} />
          </div>
        </div>

        <aside className="inspector">
          <h3>Live API state</h3>
          <dl className="state-grid">
            <div>
              <dt>Status</dt>
              <dd>{state.status}</dd>
            </div>
            <div>
              <dt>Format</dt>
              <dd>{state.format ?? "—"}</dd>
            </div>
            <div>
              <dt>Unit</dt>
              <dd>{info?.unit ?? "—"}</dd>
            </div>
            <div>
              <dt>Zoom</dt>
              <dd>{Math.round(state.zoom * 100)}%</dd>
            </div>
          </dl>
          <h3>Capabilities</h3>
          <div className="capabilities">
            {info?.capabilities ? (
              Object.entries(info.capabilities).map(([name, enabled]) => (
                <span key={name} className={enabled ? "capability-on" : ""}>
                  {name}
                </span>
              ))
            ) : (
              <span>Open a document</span>
            )}
          </div>
          <h3>Recent events</h3>
          <ol className="event-log">
            {session.events.length ? (
              session.events.map((event, index) => (
                <li key={`${event}-${index}`}>{event}</li>
              ))
            ) : (
              <li>No events yet</li>
            )}
          </ol>
        </aside>
      </div>
    </div>
  );
}

function HeadlessExample({ file }: { readonly file: File | undefined }) {
  const session = useViewerSession({ file });
  const { viewer, state, info } = session;
  const canvas = useRef<HTMLCanvasElement>(null);
  const [renderError, setRenderError] = useState("");
  const [pageText, setPageText] = useState("");
  const ready = state.status === "ready";

  useEffect(() => {
    if (!viewer || !ready || !canvas.current || !info) return;
    const controller = new AbortController();
    const target = canvas.current;
    setRenderError("");
    const render =
      info.unit === "sheet"
        ? viewer.renderSheetViewport(
            state.pageIndex,
            target,
            { row: 1, column: 1, rowCount: 30, columnCount: 12 },
            {
              width: 960,
              height: 620,
              zoom: state.zoom,
              devicePixelRatio: Math.min(devicePixelRatio, 2),
              signal: controller.signal,
            },
          )
        : viewer.renderPage(state.pageIndex, target, {
            zoom: state.zoom,
            devicePixelRatio: Math.min(devicePixelRatio, 2),
            signal: controller.signal,
          });
    void render.catch((cause: unknown) => {
      if (!isAbort(cause)) setRenderError(errorMessage(cause));
    });
    return () => controller.abort();
  }, [info, ready, state.pageIndex, state.zoom, viewer]);

  useEffect(() => setPageText(""), [file, state.pageIndex]);

  return (
    <div className="example-column">
      <ExampleIntro
        code="client.createViewer() // no DOM container"
        title="Render API, no managed DOM"
        description="React owns every element, including canvases and thumbnails. The package is used only as a parser, renderer, search and text-extraction engine."
        session={session}
      />
      <div className="headless-workspace">
        <aside className="headless-sidebar">
          <h3>Document metadata</h3>
          <pre>
            {info
              ? JSON.stringify(info, null, 2)
              : "Open a document to inspect it"}
          </pre>
          <h3>Direct API calls</h3>
          <div className="headless-actions">
            <button
              type="button"
              disabled={!ready || state.pageIndex === 0}
              onClick={() => viewer?.previous()}
            >
              Previous
            </button>
            <button
              type="button"
              disabled={!ready || state.pageIndex >= state.pageCount - 1}
              onClick={() => viewer?.next()}
            >
              Next
            </button>
            <label>
              Render scale
              <input
                type="range"
                min="0.25"
                max="2"
                step="0.05"
                value={state.zoom}
                disabled={!ready}
                onChange={(event) =>
                  viewer?.setZoom(Number(event.currentTarget.value))
                }
              />
              <output>{Math.round(state.zoom * 100)}%</output>
            </label>
            <button
              type="button"
              disabled={!ready || !info?.capabilities?.search}
              onClick={() => {
                if (!viewer) return;
                void viewer
                  .getPageText(state.pageIndex)
                  .then(setPageText)
                  .catch((cause: unknown) =>
                    setRenderError(errorMessage(cause)),
                  );
              }}
            >
              Extract current {info?.unit === "sheet" ? "sheet" : "page"} text
            </button>
          </div>
          {pageText && (
            <textarea readOnly value={pageText} aria-label="Extracted text" />
          )}
        </aside>

        <div className="headless-stage">
          <div className="headless-canvas-scroll">
            <canvas
              ref={canvas}
              className={file ? undefined : "is-empty"}
              aria-label="Headless document render"
            />
            {!file && <EmptyDocument />}
            <LoadingOverlay file={file} session={session} />
          </div>
          {(renderError || session.error) && (
            <p className="render-error" role="alert">
              {renderError || session.error}
            </p>
          )}
          {ready && info?.capabilities?.thumbnails && (
            <div className="thumbnail-strip" aria-label="Headless thumbnails">
              {Array.from(
                { length: Math.min(state.pageCount, 6) },
                (_, index) => (
                  <HeadlessThumbnail
                    key={index}
                    viewer={viewer}
                    pageIndex={index}
                    active={index === state.pageIndex}
                    onSelect={() => viewer?.goToPage(index)}
                  />
                ),
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function HeadlessThumbnail({
  viewer,
  pageIndex,
  active,
  onSelect,
}: {
  readonly viewer: ViewerApi | null;
  readonly pageIndex: number;
  readonly active: boolean;
  readonly onSelect: () => void;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (!viewer || !canvas.current) return;
    const controller = new AbortController();
    void viewer
      .renderThumbnail(pageIndex, canvas.current, {
        maxWidth: 112,
        maxHeight: 144,
        devicePixelRatio: Math.min(devicePixelRatio, 2),
        signal: controller.signal,
      })
      .catch(() => {});
    return () => controller.abort();
  }, [pageIndex, viewer]);
  return (
    <button
      type="button"
      className={active ? "thumbnail thumbnail--active" : "thumbnail"}
      aria-label={`Go to page ${pageIndex + 1}`}
      aria-current={active ? "page" : undefined}
      onClick={onSelect}
    >
      <canvas ref={canvas} />
      <span>{pageIndex + 1}</span>
    </button>
  );
}

function ExampleIntro({
  title,
  description,
  code,
  session,
}: {
  readonly title: string;
  readonly description: string;
  readonly code: string;
  readonly session: ViewerSession;
}) {
  return (
    <div className="example-intro">
      <div>
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      <code>{code}</code>
      <div className="session-status" aria-live="polite">
        <span className={`status-pill status-pill--${session.state.status}`}>
          {session.state.status}
        </span>
        <span>{session.progress}</span>
        {session.warnings.length > 0 && (
          <span title={session.warnings.join("\n")}>
            {session.warnings.length} warning(s)
          </span>
        )}
        {session.error && (
          <span className="session-error">{session.error}</span>
        )}
      </div>
    </div>
  );
}

function EmptyDocument() {
  return (
    <div className="empty-document">
      <div className="empty-icon" aria-hidden="true">
        ↑
      </div>
      <strong>Choose a document above</strong>
      <span>
        The selected file stays in the browser and is reopened when you switch
        tabs.
      </span>
    </div>
  );
}

function LoadingOverlay({
  file,
  session,
}: {
  readonly file: File | undefined;
  readonly session: ViewerSession;
}) {
  const visible =
    Boolean(file) &&
    (session.state.status === "idle" || session.state.status === "loading");
  if (!visible) return null;
  const progress =
    session.progress === "Waiting for a document"
      ? "Starting lazy-loaded parsers and renderers"
      : session.progress;
  return (
    <div className="loading-overlay" role="status" aria-live="polite">
      <span className="loading-spinner" aria-hidden="true" />
      <div>
        <strong>{loadingTitle(session.progress)}</strong>
        <span>{progress}</span>
        <small title={file?.name}>{file?.name}</small>
      </div>
    </div>
  );
}

function loadingTitle(progress: string): string {
  if (progress.startsWith("Detecting")) return "Detecting document format…";
  if (progress.startsWith("Converting"))
    return "Converting legacy Office file…";
  if (progress.startsWith("Parsing")) return "Loading renderer and parsing…";
  if (progress.startsWith("Rendering")) return "Preparing the first view…";
  return "Loading document…";
}

function formatProgress(progress: ViewerProgress): string {
  const phase = progress.phase[0]!.toUpperCase() + progress.phase.slice(1);
  return progress.ratio === undefined
    ? phase
    : `${phase} · ${Math.round(progress.ratio * 100)}%`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function isAbort(cause: unknown): boolean {
  return cause instanceof ViewerError && cause.code === "aborted";
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
