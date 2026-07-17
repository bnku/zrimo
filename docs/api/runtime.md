# Runtime API

The runtime exposes one shared `ViewerClient` and one or more `DocumentViewer` instances. Imports are SSR-safe: construction does not touch the DOM, start workers, initialize WASM, or make network requests.

```ts
import { ViewerClient } from "@zrimo/viewer";

const client = ViewerClient.create({
  assetBaseUrl: new URL("/viewer-assets/", location.href),
  fetch: window.fetch.bind(window),
  limits: { maxInputBytes: 50 * 1024 * 1024 },
});

const viewer = client.createViewer({ locale: "ru", fit: "width" });
await viewer.load(file, { fileName: file.name, signal });
```

## Lifecycle

`ViewerClient.create(options)` owns the adapter registry and shared hooks. `client.createViewer(options)` creates an isolated document lifecycle. `load` and its compatibility alias `open` close the previous document, load and limit-check the source, detect content, select an adapter, open it, and publish `ready` only after `getInfo` succeeds.

A newer `load` cancels an older in-flight load. `close` cancels work, calls the adapter exactly once, releases the original bytes, and returns to `idle`. `destroy` is idempotent, performs the same cleanup, removes listeners, and makes later commands fail with `lifecycle-error`. Destroying a client destroys all of its viewers and unique adapters.

The original input is retained only while the document is open so the future download command can return the exact source. `getOriginalBytes()` returns a defensive copy and returns `undefined` after `close`.

## Sources and network policy

Accepted sources are `string`, `URL`, `Blob`/`File`, `ArrayBuffer`, and `Uint8Array`. Blob size and HTTP `Content-Length` are checked before body allocation where available; streaming responses are counted per chunk and aborted when they cross `maxInputBytes`.

URL loading uses only the `fetch` function supplied to `ViewerClient`, or the bound global `fetch` when no hook is supplied. CORS failures, transport failures, and non-2xx responses become `network-error`. No adapter receives permission for implicit network calls; `assetBaseUrl` is an explicit base for package-owned WASM/worker/font assets only.

## Detection

Content signatures win over filename and MIME hints. The detector recognizes PDF, PNG, JPEG, GIF, WebP, BMP, TIFF, SVG, OOXML ZIP families, and OLE stream names for DOC/XLS/PPT. Macro-enabled OOXML and PPSX subtype hints are preserved only when their family agrees with the container. CSV/TSV require an explicit extension, MIME, or format hint because arbitrary text is ambiguous.

A mismatched extension or MIME emits `format-hint-mismatch`; it does not force the wrong parser. Unknown content returns `unsupported-format`.

## Events

- `statechange`: immutable lifecycle/navigation snapshot.
- `ready`: the adapter opened and document info is available.
- `progress`: loading, detecting, parsing, converting, or rendering progress.
- `warning`: typed non-fatal fidelity/security warning.
- `error`: serializable `ViewerErrorData`, never an upstream-specific exception.
- `pagechange`: current zero-based page/slide/image/sheet changed.
- `zoomchange`: zoom or fit mode changed.
- `viewchange`: coalesced view state, at most once per animation frame.
- `searchchange`: immutable search result or `null`.
- `selectionchange`: logical text selection, spreadsheet cell range, or `null`.

The complete public contract and headless examples are in [the API reference](./reference.md) and [headless guide](./headless.md).

## Adapter contract

`DocumentAdapter` declares a stable `id` and formats and implements `open`, `getInfo`, `render`, optional `getTextMap`, `close`, and optional shared `destroy`. `AdapterOpenContext` carries the detected format, limits, signal, asset base, and progress/warning callbacks.

`WorkerDocumentAdapter` is the default worker bridge for forthcoming format adapters. It transfers a copy of the input buffer to an ESM worker, retains the original on the main side, requests bitmap/text results through `WorkerRpcClient`, closes `ImageBitmap`s after 2D drawing, and terminates the worker on close or failure.

## Error catalog

Stable codes are `unsupported-format`, `fidelity-unsupported`, `invalid-file`, `encrypted-document`, `resource-limit`, `network-error`, `aborted`, `font-unavailable`, `render-failed`, `worker-crashed`, `lifecycle-error`, and `internal`. `fidelity-unsupported` distinguishes a recognized but unqualified format/revision from an unknown format when the available backend cannot preserve material structure. Worker errors cross the boundary as `{ name: "ViewerError", code, message, details? }`; stack traces and arbitrary causes are not serialized.
