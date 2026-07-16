# Performance and resource budgets

## Reference host

The 2026-07-16 release-candidate baseline was recorded on Linux 6.12 x86_64, AMD Ryzen 9 5950X, 94 GiB RAM, Node 24.18.0, Rust 1.94.1 and system Chromium 145. This is a regression host, not a promise that every end-user device has identical latency.

## Automated scenario

`npm run test:e2e` loads a generated 10 MiB PDF-shaped source through the public API, waits for the first visible render, then performs 40 frame-separated pan/zoom operations on a virtualized 1,000-page document. The gate requires:

- first visible render at or below 2,500 ms;
- no observed interaction long task above 50 ms;
- no more than five live page canvases;
- repeated load/close plus final destroy to release every handle;
- a timed-out worker operation to terminate its Worker.

The final local result is 5.6 ms to first render, 0 ms maximum observed long task and two live canvases. The machine-readable output is `artifacts/performance-chromium.json`. Production parsers are additionally exercised by the format qualification flows; the synthetic fixture isolates viewer/runtime overhead from document complexity.

## Rendering and memory controls

- Visible renders outrank adjacent pages, which outrank thumbnails. Only two render jobs run concurrently by default; queued stale work is aborted.
- Page DOM is virtualized with configurable overscan. PDF raster cache, decoded pixels, text-map bytes and document unit counts have hard limits.
- Every built-in worker operation has a 30-second default budget. Timeout returns `resource-limit` and terminates the worker/WASM heap.
- PDF cache evicts least-recently-used bitmaps before accepting a page that would cross its pixel budget. Close/destroy clears caches, listeners, object URLs and workers.

Applications can lower or explicitly raise limits through `ViewerClient.create({ limits })`. Raising them increases peak-memory and denial-of-service exposure; it should be based on a host-specific benchmark.

## Bundle budget

Run `npm run build && npm run report:size`. The report includes raw, gzip and Brotli bytes per JS/CSS/worker/WASM asset and records fonts separately.

| Delivery set | Raw | Gzip | Brotli |
|---|---:|---:|---:|
| Base code + all lazy WASM modules | 9.97 MiB | 4.30 MiB | 3.26 MiB |
| Optional Noto font packs | 10.46 MiB | 10.46 MiB | 10.45 MiB |
| Package assets with every font | 20.43 MiB | 14.76 MiB | 13.71 MiB |

The release target is 20 MiB Brotli for base code, excluding optional script-specific fonts; 20–25 MiB requires an explanation and above 25 MiB blocks release. Fonts and format WASM are lazy assets, so normal document loads transfer only the required subset.
