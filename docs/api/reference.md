# Public API reference

`@docs-viewer-wasm/viewer` is an ESM, browser-side document viewer. Importing the package and creating a headless viewer are SSR-safe; a DOM is required only when `container` is supplied or `downloadOriginal()` initiates a browser download.

All page, slide, image, and sheet indices are zero-based. API snapshots and result arrays are immutable.

## Create a runtime and viewer

```ts
import { ViewerClient } from "@docs-viewer-wasm/viewer";

const client = ViewerClient.create({
  assetBaseUrl: new URL("/document-viewer/", location.href),
  limits: { maxInputBytes: 50 * 1024 * 1024 },
});

const viewer = client.createViewer({
  container: document.querySelector<HTMLElement>("#viewer")!,
  locale: "ru",
  layout: "continuous",
  fit: "width",
  overscan: 1,
});

await viewer.load(file, { fileName: file.name });
```

`ViewerClient.create()` and `createViewer()` are synchronous. Parsing and rendering assets are initialized lazily by `load()`/render operations. A client may own multiple independent viewers.

### `ViewerClientOptions`

| Option         | Type                         | Meaning                                                                                        |
| -------------- | ---------------------------- | ---------------------------------------------------------------------------------------------- |
| `assetBaseUrl` | `string \| URL`              | Base URL for package-owned workers, WASM, and future font assets.                              |
| `fetch`        | `ViewerFetch`                | Host-controlled fetch implementation used only for explicit URL sources.                       |
| `logger`       | `ViewerLogger`               | Optional debug/warning/error sink.                                                             |
| `limits`       | `Partial<ResourceLimits>`    | Runtime defaults for input/archive/pixel/SVG/CSV/text/unit/concurrency/operation-time budgets. |
| `adapters`     | `readonly DocumentAdapter[]` | Replace the built-in adapter set, primarily for testing or custom formats.                     |
| `fontPolicy`   | `FontPolicy`                 | Select `auto`, `offline`, or host `custom` font resolution.                                    |
| `fonts`        | `readonly RegisteredFont[]`  | App-provided URL/byte faces resolved before system/package fallback.                           |

`client.registerAdapter(adapter)` registers an additional adapter and returns an unregister function. `await client.destroy()` destroys every owned viewer and shared adapter.

### `ViewerOptions`

| Option         | Default         | Meaning                                                                                         |
| -------------- | --------------- | ----------------------------------------------------------------------------------------------- |
| `container`    | none            | `HTMLElement` receiving the managed viewport; omit for headless use.                            |
| `layout`       | `"continuous"`  | Continuous virtual list or one visible unit with `"single"`.                                    |
| `overscan`     | `1`             | Extra units on either side of the visible range, clamped to `0…5`.                              |
| `initialZoom`  | `1`             | Initial scale, clamped to `0.1…8`.                                                              |
| `fit`          | `"none"`        | Initial `"none"`, `"width"`, or `"page"` fit mode.                                              |
| `locale`       | `"en"`          | Built-in locale identifier (`"en"` or `"ru"`).                                                  |
| `ui`           | `false`         | Mount the optional localized toolbar/panels around the viewport.                                |
| `useWorker`    | adapter default | Compatibility switch for integrations; built-in parsing backends select their safe worker path. |
| `translations` | none            | Partial locale dictionary override with English fallback.                                       |

The host must give the container a resolvable height. The viewport fills its container and removes all DOM/listeners during `destroy()`.

## Lifecycle and state

`load(source, options?)` accepts a URL string, `URL`, `Blob`/`File`, `ArrayBuffer`, or `Uint8Array`. `open()` is an alias. A new load cancels the previous load and closes the old handle. `close()` returns the viewer to `idle`; `destroy()` is idempotent and terminal.

```ts
interface ViewerState {
  readonly status: "idle" | "loading" | "ready" | "error" | "destroyed";
  readonly format?: DocumentFormat;
  readonly pageIndex: number;
  readonly pageCount: number;
  readonly zoom: number;
  readonly fit: "none" | "page" | "width";
  readonly panX: number;
  readonly panY: number;
}
```

`viewer.state` is replaced, not mutated. `getDocumentInfo()` returns format,
render unit, count, optional sheet geometry/warnings, and capabilities
(`textSelection`, `cellSelection`, `search`, and `thumbnails`). Page-oriented
backends may also return `pageSizes`, an ordered `{ width, height }[]` in natural
CSS pixels at zoom 1; the managed viewport uses it for mixed-size PDF and Office
documents instead of coercing every page to A4.

## View and navigation

| Method                     | Behavior                                                                            |
| -------------------------- | ----------------------------------------------------------------------------------- |
| `setZoom(zoom)`            | Set a clamped absolute zoom and leave fit mode.                                     |
| `zoomIn()` / `zoomOut()`   | Multiply or divide zoom by `1.2`.                                                   |
| `setFit(mode)`             | Apply `none`, `width`, or `page`.                                                   |
| `fitWidth()` / `fitPage()` | Derive scale from the attached viewport; headless viewers keep their current scale. |
| `panBy(dx, dy)`            | Scroll the attached viewport or update headless pan state.                          |
| `goToPage(index)`          | Navigate to a clamped page/slide/image/sheet index.                                 |
| `next()` / `previous()`    | Navigate one unit.                                                                  |
| `setSheet(index)`          | Navigate a sheet document; throws `lifecycle-error` for non-sheet documents.        |

The managed viewport also supports pointer drag, Ctrl/Cmd+wheel, two-pointer
pinch, Page Up/Down, and arrow-key pan. Spreadsheet documents automatically use
a virtual surface with variable/hidden row and column geometry, fixed
headers/frozen panes, one bounded canvas, and enough trailing blank bands to
fill the viewport. Sheet `fitWidth` fits the
used columns, `fitPage` fits both axes, and zoom preserves the pointer or center
anchor. Spreadsheet cells support pointer drag, Shift+arrow range extension,
and Ctrl/Cmd+C copying of the current range.

The optional controls, shortcuts, localization, and CSS variables are documented in [Basic UI](../ui.md). Font policies and resolver types are documented in [Fonts](../fonts.md).

## Search and selection

`search(query, { caseSensitive? })` searches logical text in page order. The default applies Unicode NFKC normalization and locale-independent case folding while mapping matches back to original UTF-16 offsets. It does not strip Arabic diacritics or reorder RTL text.

```ts
const result = await viewer.search("привет");
viewer.searchNext();
viewer.searchPrevious();
viewer.clearSearch();
```

`SearchResult` contains the original query, immutable `{ pageIndex, start, end, text }` matches, and `activeIndex` (`-1` when empty). Starting a new search cancels an earlier one.

Use `selectText({ startPageIndex, startOffset, endPageIndex, endOffset })` for logical cross-page ranges. Use `selectCells({ sheetIndex, startRow, startColumn, endRow, endColumn })` for spreadsheets; merged cells expand the returned range. `copySelection()` writes to the Clipboard API when permission is available and always resolves to the deterministic text/TSV value. `getSelection()` and `clearSelection()` expose the current model.

Text/search offsets are UTF-16 code-unit offsets. Programmatic ranges preserve
that contract exactly; native drag/double-click endpoints are grapheme-aware so
the UI does not split emoji, combining sequences, or Indic clusters. DOCX
selectable spans use the renderer's exact font, pitch, rotation, and vertical
metadata. Search highlights live in a separate `pointer-events:none` overlay and
cannot change native selection geometry.

## Headless rendering

| Method                                                | Result                                                                                       |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `renderPage(index, canvas, options?)`                 | Render one page/slide/sheet/image into an existing `HTMLCanvasElement` or `OffscreenCanvas`. |
| `renderThumbnail(index, canvas, options?)`            | Render using a scale bounded by `maxWidth`/`maxHeight`.                                      |
| `renderSheetViewport(index, canvas, range, options?)` | Render a bounded row/column region.                                                          |
| `getPageText(index, signal?)`                         | Return logical text for one render unit.                                                     |

Render options are `zoom`, `devicePixelRatio`, optional `width`/`height`, and
`signal`. Sheet-region rendering additionally accepts unscaled
`scrollOffsetX/Y` for a partially clipped first column/row. Operations reject
stale completion after close/reload and normalize cancellation to `ViewerError`
with code `aborted`. See [headless rendering](./headless.md).

## Original bytes

`getOriginalBytes()` returns a defensive copy while a document is open. `downloadOriginal(fileName?)` returns a `Blob` containing the exact original bytes and, in a browser document, also starts a download. Legacy Office conversion output is never substituted for the original.

## Events

`viewer.on(type, listener)` returns an unsubscribe function.

| Event             | Payload                                                               |
| ----------------- | --------------------------------------------------------------------- |
| `statechange`     | Complete immutable `ViewerState`.                                     |
| `ready`           | Ready `ViewerState`.                                                  |
| `pagechange`      | `{ pageIndex, pageCount }`.                                           |
| `zoomchange`      | `{ zoom, fit }`.                                                      |
| `viewchange`      | Coalesced `ViewerState`, at most once per animation frame.            |
| `progress`        | Loading/detecting/parsing/converting/rendering progress.              |
| `searchchange`    | `SearchResult \| null`.                                               |
| `selectionchange` | `TextSelection \| CellRange \| null`.                                 |
| `warning`         | Non-fatal `ViewerWarning`.                                            |
| `error`           | Serializable `ViewerErrorData`. Promise-returning calls still reject. |

## Errors and warnings

Catch `ViewerError` and branch on its stable `code`: `unsupported-format`, `fidelity-unsupported`, `invalid-file`, `encrypted-document`, `resource-limit`, `network-error`, `aborted`, `font-unavailable`, `render-failed`, `worker-crashed`, `lifecycle-error`, or `internal`. `fidelity-unsupported` remains available for a recognized format/revision that no qualified backend can render without inventing or losing material structure. Qualified Word 97–2003 DOC uses the structured conversion path; older or malformed revisions fail typed conversion instead of falling back to plain text.

Warnings use `format-hint-mismatch`, `unsupported-feature`, `font-substitution`, `external-resource-blocked`, or `fidelity-degraded`. Warnings report explicit degradation and do not silently enable active content.
