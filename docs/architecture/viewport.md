# Viewport architecture

`AdaptiveViewport` selects one of two layout strategies over the shared
`DocumentViewer` state and adapter API. `ViewerViewport` handles pages, slides,
images, and SVG; `SpreadsheetViewport` handles XLSX/XLSM/XLS and CSV/TSV. A
sheet never enters page-slot or A4 extent calculations.

## Coordinate spaces

1. Backend coordinates are unscaled logical CSS pixels returned by each adapter.
2. View coordinates multiply logical positions by `ViewerState.zoom`.
3. Device pixels multiply the render request by `devicePixelRatio`.
4. Scroll coordinates (`panX`, `panY`) belong to the active viewport root.

For pages, the logical extent is the page box. For sheets, separate sparse axis
indexes describe 1-based rows and columns. Defaults plus custom/hidden band
sizes form prefix sums; binary search maps a scroll offset back to a cell
without scanning the used range.

Text runs retain logical order and independently carry visual `x/y/width/height`
plus `ltr`/`rtl` direction. Format backends may additionally supply exact CSS
`font`, `fontSize`, `letterSpacingPx`, `transform`, vertical-run flags, the
unscaled coordinate extent, and explicit logical UTF-16 offsets. Copy/search
never reconstruct text by sorting visual coordinates. Spreadsheet
`TextRun.row/column` values connect canvas geometry to the cell-selection model.

## Continuous virtualization

The spacer represents the total logical extent, but the DOM contains only the visible interval plus `0…5` overscan units. `visibleRange()` calculates a clamped half-open interval from scroll offset, viewport height, unit extent, and document count. Leaving slots are aborted and removed; entering slots receive one canvas and one transparent logical text layer.

Consequently, a 10,000-page document keeps the same bounded canvas/DOM population as a short document. The Chromium interaction test asserts this invariant after jumping into the middle of the document.

`single` layout mounts only `state.pageIndex`. Page, slide, and image navigation
use the same zero-based methods and events. Sheet tabs also use `pageIndex`, but
switch the spreadsheet model and restore that sheet's saved scroll position.

## Render scheduling and zoom preview

State changes coalesce viewport work through `requestAnimationFrame`. Every slot owns an `AbortController`, generation counter, and render key containing zoom, DPR, and search-highlight state. A changed or recycled slot aborts stale rendering; late completion cannot overwrite the current slot.

During Ctrl/Cmd+wheel or two-pointer pinch, the existing canvas stretches with the slot for an immediate preview. Crisp backend renders are deferred until 120 ms after the last zoom gesture. Programmatic zoom renders on the next frame. Continuous zoom rescales the scroll offset by old/new unit extent, retaining the same logical document position instead of jumping to another page.

A `ResizeObserver` schedules fit/layout updates, and DPR participates in the render key so a DPR change invalidates visible output. Backends own format-specific bounded caches: for example, the PDF adapter uses an LRU pixel cache. The viewport itself does not retain off-screen canvases.

## Spreadsheet behavior

The sheet spacer is the complete used-range width and height, including custom
and zero-sized hidden bands. It contains one sticky, viewport-sized canvas and
one selection overlay—never a DOM element per cell. Scroll plus frozen-pane
geometry determines a bounded `{ row, column, rowCount, columnCount }` render
request with two-band overscan. `scrollOffsetX/Y` preserves partially clipped
first cells, while the backend keeps headers and frozen rows/columns fixed.

Every render uses a detached frame canvas, an `AbortController`, and a
generation token. Only the newest completed frame is copied to the visible
canvas, so an adapter that ignores cancellation still cannot overwrite a newer
scroll/zoom/resize result. The visible canvas remains bounded to the viewport at
all zoom levels.

`fitWidth` fits the entire used width; `fitPage` fits both used dimensions.
Pointer zoom retains the logical point under the cursor, programmatic zoom
retains the logical point at the viewport's top-left corner, and `panBy` remains
ordinary CSS-pixel scrolling.
Pointer hit testing and Shift+arrow use the same axis indexes as rendering.
Merged-range expansion happens in `DocumentViewer`, and copy materializes TSV
from the complete logical cell map, including sparse cells beyond the initial
viewport.

## Search and selection overlays

Each page slot has a canvas, an inert highlight layer, and a selectable text
layer. Highlights never change span backgrounds or pointer hit testing. The
generic overlay remains available for simple positioned maps, while DOCX uses
`buildDocxTextLayer`/`buildDocxHighlightLayer` from the same pinned renderer that
produced its run geometry. The natural coordinate layer is transformed as a
unit for zoom, so font metrics, letter spacing, rotation, vertical text, and
DPR stay aligned with the canvas. Loading is lazy and slot generation/abort
checks prevent a late overlay from replacing a newer render.

Search builds a normalized logical index outside the viewport. Only matches for mounted units are projected into their highlight layers. A search change updates render keys without retaining pages outside the virtual range.

Native browser selection endpoints are translated from span-local DOM offsets
to page-level logical offsets and then to a cross-page `TextSelectionRange`.
Offsets in the API remain UTF-16 for compatibility. Native UI endpoints that
land inside an emoji/Indic/combining sequence are expanded to the nearest
grapheme boundary and the DOM Range is corrected accordingly. Programmatic
selection continues to accept exact UTF-16 offsets. Mixed RTL/LTR content
therefore copies in backend logical order rather than visual coordinate order.

## Lifecycle

Attaching a new document clears and aborts all slots. `close()` clears the document while keeping the viewport reusable. `destroy()` cancels animation/timers/renders, disconnects `ResizeObserver`, removes scroll/wheel/pointer/keyboard/selection listeners, removes every slot, and detaches the root element.
