# Migration from `@docmentis/udoc-viewer`

This project deliberately follows the familiar client → viewer → `load()` shape of `@docmentis/udoc-viewer`, but it is not a drop-in replacement. This note compares against the public `@docmentis/udoc-viewer` 0.7.9 README as observed on 2026-07-16; verify upstream release notes when migrating from a later version.

## Minimal setup

```ts
// @docmentis/udoc-viewer
const client = await UDocClient.create({ baseUrl: "/udoc/" });
const viewer = await client.createViewer({ container: "#viewer" });
await viewer.load(source);
```

```ts
// @docs-viewer-wasm/viewer
const client = ViewerClient.create({
  assetBaseUrl: new URL("/document-viewer/", location.href),
});
const viewer = client.createViewer({
  container: document.querySelector<HTMLElement>("#viewer")!,
});
await viewer.load(source, { fileName: "report.docx" });
```

The new client/viewer constructors are synchronous; parsing and WASM initialization remain lazy and asynchronous. `container` is an `HTMLElement`, not a selector string. Supplying `fileName` is important for ambiguous text formats such as CSV/TSV and useful when source bytes have no URL.

## Main mappings

| `@docmentis/udoc-viewer` concept | This package                                         |
| -------------------------------- | ---------------------------------------------------- |
| `UDocClient.create()`            | `ViewerClient.create()` (synchronous)                |
| `baseUrl`                        | `assetBaseUrl`                                       |
| async `client.createViewer()`    | synchronous `client.createViewer()`                  |
| selector or element container    | `HTMLElement` container                              |
| `searchPrev()`                   | `searchPrevious()`                                   |
| viewer-owned render result       | caller-owned canvas passed to `renderPage()`         |
| built-in source download         | `downloadOriginal()` preserving exact input bytes    |
| page events/state                | typed `ViewerEventMap` plus immutable `viewer.state` |

Both libraries use zero-based page indices and provide pan/zoom, fit, search, page navigation, headless rendering, browser-only document processing, and framework-neutral lifecycle cleanup.

## Behavioral differences

- This package has no telemetry, permit verification, license-key check, attribution enforcement, or implicit third-party font request. URL fetches are limited to a source explicitly supplied by the host and package assets under `assetBaseUrl`.
- The runtime and shipped dependencies are selected through a permissive-license gate intended for closed commercial applications. Consult [dependencies](./dependencies.md) for the audited inventory.
- XLS/PPT legacy binaries use in-browser legacy-to-OOXML normalization. DOC is recognized but returns `fidelity-unsupported`, because the current permissive browser parser cannot preserve Word Binary formatting and tables. The exact original bytes remain available for download for successfully opened inputs.
- Macro-enabled OOXML is rendered without executing VBA. Spreadsheet formulas use stored cached values and are not calculated.
- Password-protected documents return `encrypted-document`; password authentication is outside v1.
- Annotations, editing, rotation/region-render helpers, and custom page overlays are outside the current v1 scope.
- Headless rendering writes into a host-provided `HTMLCanvasElement`/`OffscreenCanvas`; convert it to `Blob`, `ImageData`, or a data URL with standard Canvas APIs.
- `search()` returns an immutable `SearchResult` with original logical offsets. Selection and clipboard APIs also expose spreadsheet cell ranges/TSV.

The optional basic toolbar/panels are a separate layer from this stable headless contract. Applications can integrate the API without mounting that UI.

## Cleanup

Both viewers require explicit cleanup. Awaiting cleanup is recommended here because adapters may terminate workers or release WASM handles asynchronously:

```ts
await viewer.destroy();
await client.destroy();
```

Upstream reference: [npm package README](https://www.npmjs.com/package/@docmentis/udoc-viewer).
