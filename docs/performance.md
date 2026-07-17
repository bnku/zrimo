# Performance and resource budgets

Zrimo keeps work close to the visible viewport. Page documents render a small
window of pages and spreadsheets render a canvas-sized cell range instead of a
full sheet. Format engines, workers and optional font packs are loaded only
when the opened document requires them.

Actual first-render time depends on document structure, compressed size,
embedded media, selected fonts, device performance and browser canvas/WASM
implementations. Applications with a strict latency target should benchmark
representative documents on their own device matrix.

## Rendering and memory controls

- Visible renders outrank adjacent pages, which outrank thumbnails. Only two render jobs run concurrently by default; queued stale work is aborted.
- Page DOM is virtualized with configurable overscan. PDF raster cache, decoded pixels, text-map bytes and document unit counts have hard limits.
- Every built-in worker operation has a 30-second default budget. Timeout returns `resource-limit` and terminates the worker/WASM heap.
- PDF cache evicts least-recently-used bitmaps before accepting a page that would cross its pixel budget. Close/destroy clears caches, listeners, object URLs and workers.

Applications can lower or explicitly raise limits through `ViewerClient.create({ limits })`. Raising them increases peak-memory and denial-of-service exposure; it should be based on a host-specific benchmark.

## Delivery size

The npm package contains all supported format modules and optional fallback
fonts, but a normal document does not download all of them. Configure
`assetBaseUrl` so workers, the required WASM adapter and only the encountered
script-specific font packs can be fetched independently and cached by the
browser.

Avoid copying runtime assets into JavaScript bundles or importing every worker
eagerly. The provided `zrimo-copy-assets` command preserves the directory
layout expected by the lazy loader.

## Integration guidance

- Keep the viewer container size stable while a document is open; repeated
  layout changes can invalidate visible canvases.
- Call `destroy()` when unmounting so workers, object URLs, canvases and caches
  are released promptly.
- Lower limits for untrusted public uploads and raise them only after measuring
  memory use on the weakest supported device.
- Serve `.wasm`, `.mjs` and font files with correct MIME types and long-lived,
  versioned cache headers.
