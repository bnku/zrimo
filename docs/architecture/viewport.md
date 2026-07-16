# Viewport architecture

The viewport is a format-neutral projection over `DocumentAdapter.render()` and `getTextMap()`. PDF pages, Office pages/slides/sheets, raster frames, SVG, and delimited data share the same state, scheduling, cancellation, and interaction model.

## Coordinate spaces

1. Backend coordinates are logical units returned by each adapter's canvas/text map.
2. View coordinates multiply logical positions by `ViewerState.zoom`.
3. Device pixels multiply the render request by `devicePixelRatio`.
4. Scroll coordinates (`panX`, `panY`) belong to the viewport root.

Text runs retain logical order and independently carry visual `x/y/width/height` plus `ltr`/`rtl` direction. Copy/search never reconstruct text by sorting visual coordinates. Spreadsheet `TextRun.row/column` values connect canvas geometry to the cell-selection model.

## Continuous virtualization

The spacer represents the total logical extent, but the DOM contains only the visible interval plus `0…5` overscan units. `visibleRange()` calculates a clamped half-open interval from scroll offset, viewport height, unit extent, and document count. Leaving slots are aborted and removed; entering slots receive one canvas and one transparent logical text layer.

Consequently, a 10,000-page document keeps the same bounded canvas/DOM population as a short document. The Chromium interaction test asserts this invariant after jumping into the middle of the document.

`single` layout mounts only `state.pageIndex`. Page, slide, image, and sheet navigation use the same zero-based methods and events.

## Render scheduling and zoom preview

State changes coalesce viewport work through `requestAnimationFrame`. Every slot owns an `AbortController`, generation counter, and render key containing zoom, DPR, and search-highlight state. A changed or recycled slot aborts stale rendering; late completion cannot overwrite the current slot.

During Ctrl/Cmd+wheel or two-pointer pinch, the existing canvas stretches with the slot for an immediate preview. Crisp backend renders are deferred until 120 ms after the last zoom gesture. Programmatic zoom renders on the next frame. Continuous zoom rescales the scroll offset by old/new unit extent, retaining the same logical document position instead of jumping to another page.

A `ResizeObserver` schedules fit/layout updates, and DPR participates in the render key so a DPR change invalidates visible output. Backends own format-specific bounded caches: for example, the PDF adapter uses an LRU pixel cache. The viewport itself does not retain off-screen canvases.

## Spreadsheet behavior

The public `renderSheetViewport()` sends a bounded `{ row, column, rowCount, columnCount }` region to the adapter. The attached sheet surface remains canvas-based; it does not create a table DOM node for every cell. Positioned text runs provide visible cell hit targets. Pointer drag and Shift+arrow update a logical `CellRange`; merged-range expansion happens in `DocumentViewer`, and copy materializes TSV from the logical cell map.

## Search and selection overlays

Search builds a normalized logical index outside the viewport. Only matches for mounted units are projected into their text layers. A search change updates render keys without retaining pages outside the virtual range.

Native browser selection endpoints are translated from span-local DOM offsets to page-level logical offsets and then to a cross-page `TextSelectionRange`. Programmatic selection follows the identical model. Mixed RTL/LTR content therefore copies in document order.

## Lifecycle

Attaching a new document clears and aborts all slots. `close()` clears the document while keeping the viewport reusable. `destroy()` cancels animation/timers/renders, disconnects `ResizeObserver`, removes scroll/wheel/pointer/keyboard/selection listeners, removes every slot, and detaches the root element.

