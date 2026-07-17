# Headless rendering, search, and selection

A headless `DocumentViewer` has the same parser, render, search, selection, cancellation, and resource-limit behavior as an attached viewer, but creates no DOM viewport.

## Render into a canvas

```ts
import { ViewerClient, ViewerError } from "@docs-viewer-wasm/viewer";

const client = ViewerClient.create({
  assetBaseUrl: new URL("/document-viewer/", location.href),
});
const viewer = client.createViewer();

await viewer.load(bytes, { fileName: "report.pdf" });

const canvas = document.createElement("canvas");
await viewer.renderPage(0, canvas, {
  zoom: 1.5,
  devicePixelRatio: window.devicePixelRatio,
});
document.body.append(canvas);
```

The caller owns the target canvas. The backend sets its pixel dimensions; `zoom` is the logical scale and `devicePixelRatio` controls output density. Page indices are zero-based.

## OffscreenCanvas and cancellation

```ts
const controller = new AbortController();
const target = new OffscreenCanvas(1, 1);

const pending = viewer.renderPage(3, target, {
  zoom: 2,
  devicePixelRatio: 1,
  signal: controller.signal,
});

controller.abort();

try {
  await pending;
} catch (error) {
  if (!(error instanceof ViewerError) || error.code !== "aborted") throw error;
}
```

`close()`, `destroy()`, and a newer `load()` also cancel active renders and reject stale results. An external signal does not take ownership of the viewer.

## Thumbnails

```ts
const thumbnail = document.createElement("canvas");
await viewer.renderThumbnail(0, thumbnail, {
  maxWidth: 180,
  maxHeight: 240,
  devicePixelRatio: 2,
});
```

Thumbnail scaling never enlarges past the package base scale. Spreadsheet documents expose bounded region rendering instead of whole-sheet thumbnails.

## Spreadsheet viewport

```ts
const sheetCanvas = document.createElement("canvas");
await viewer.renderSheetViewport(
  0,
  sheetCanvas,
  { row: 100, column: 20, rowCount: 40, columnCount: 12 },
  {
    width: 960,
    height: 640,
    devicePixelRatio: 2,
    // Unscaled pixels clipped inside row 100 / column 20:
    scrollOffsetX: 18,
    scrollOffsetY: 6,
  },
);
```

Rows and columns in both `SpreadsheetViewportRange` and `CellRange` are one-based spreadsheet coordinates; `sheetIndex` remains zero-based.
The attached `SpreadsheetViewport` calculates this range and the partial-cell
offsets from sparse row/column geometry. A custom headless scroller should use
the same convention; offsets do not include frozen panes or headers.

## Search without UI

```ts
const { matches } = await viewer.search("日本語");
for (const match of matches) {
  console.log(match.pageIndex, match.start, match.end, match.text);
}

const pageText = await viewer.getPageText(0);
```

Matches point into the original logical UTF-16 text even when NFKC/case folding changed the search representation. Search is literal, not fuzzy, and preserves Arabic diacritics.

## Programmatic selection and copy

```ts
await viewer.selectText({
  startPageIndex: 0,
  startOffset: 10,
  endPageIndex: 1,
  endOffset: 24,
});
const text = await viewer.copySelection();

viewer.selectCells({
  sheetIndex: 0,
  startRow: 2,
  startColumn: 1,
  endRow: 8,
  endColumn: 4,
});
const tsv = await viewer.copySelection();
```

The returned string does not depend on Clipboard permission. Text follows backend logical order rather than visual RTL/LTR placement; cell selection serializes rows as TSV and expands merged ranges.

## Cleanup

```ts
await viewer.close();   // reusable viewer, status becomes idle
await viewer.destroy(); // terminal and idempotent
await client.destroy(); // destroys any viewers still owned by the client
```
