# @zrimo/viewer

**Any document. One canvas.**

Zrimo is an embeddable browser document viewer with an SSR-safe TypeScript facade and lazy Rust/WASM format adapters. No server conversion or telemetry is used.

```bash
npm install @zrimo/viewer
npx zrimo-copy-assets public/zrimo
```

```ts
import { ViewerClient } from "@zrimo/viewer";
import "@zrimo/viewer/styles.css";

const client = ViewerClient.create({
  assetBaseUrl: new URL("/zrimo/", location.href),
  limits: { maxInputBytes: 50 * 1024 * 1024 },
});
const viewer = client.createViewer({
  container: document.querySelector("#viewer")!,
  locale: "ru",
  ui: true,
  fit: "width",
});
await viewer.load(file, { fileName: file.name });
```

Supported render formats are DOCX/DOCM, XLSX/XLSM, PPTX/PPTM/PPSX, Word 97–2003 DOC, legacy XLS/PPT, PDF, PNG/JPEG/WebP/GIF/BMP/TIFF, SVG and CSV/TSV. DOC is parsed by a bounded project-owned Rust backend, serialized in memory to DOCX, and then uses the same page renderer and selectable text layer as DOCX. Source comment authors, bodies, point anchors and qualified range anchors are preserved. Other advanced Word Binary features such as Word 6, OLE objects, complex floating shapes and some image/table variants remain unsupported or fidelity-degraded. Search, selection, pan/zoom, page/sheet navigation, thumbnails and original download are available through the same API. Images do not include OCR; encrypted documents and formula/macro execution are outside v1.

Sheets use a dedicated virtual spreadsheet surface: the scrollbar covers the
complete used range and the visible trailing blank rows/columns, while only one
viewport-sized canvas is rendered. Custom and hidden band sizes, frozen panes,
sparse far-away cells, zoom `0.1…8`, cell selection, and TSV copy through
`copySelection()` or Ctrl/Cmd+C do not depend on an A4 page box. Column-header
borders can be dragged to apply view-only, per-sheet width overrides. The
vanilla example's `?large-sheet=1` mode creates a synthetic 250×100 CSV to
demonstrate deep horizontal and vertical scrolling without shipping a fixture.
Shift+click extends the active cell range; Ctrl/Cmd+click or drag builds a
non-contiguous selection that is copied in row-major TSV order.

Subpath exports:

- `@zrimo/viewer` — complete public API and optional UI;
- `@zrimo/viewer/headless` — UI-free runtime/adapters;
- `@zrimo/viewer/worker` — custom worker adapter/RPC contracts;
- `@zrimo/viewer/styles.css`, `/fonts/*`, `/workers/*`, `/assets/*` — explicit assets.

The browser baseline is Safari 16.4+ and the latest two stable Chrome, Edge, Firefox and Safari releases. See the repository documentation for API, compatibility, security, self-hosting and release artifacts.

## Built on open source

Zrimo stands on the work of excellent open-source projects. Thank you to every
maintainer and contributor who made these building blocks available.

| Project                                                                                                                                              | License                   | How Zrimo uses it                                                                                                                                       |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Office Open XML Viewer (`@silurus/ooxml`)](https://github.com/yukiyokotani/office-open-xml-viewer)                                                  | MIT                       | Rust/WASM parsing and Canvas rendering for DOCX, XLSX and PPTX.                                                                                         |
| [`office_oxide`](https://github.com/yfedoseev/office_oxide)                                                                                          | MIT OR Apache-2.0         | Compound-file handling, shared Office IR/writer utilities and browser-side legacy XLS/PPT conversion. Zrimo's Word Binary parser remains project-owned. |
| [Mozilla PDF.js](https://github.com/mozilla/pdf.js)                                                                                                  | Apache-2.0                | PDF parsing, font/CMap handling, Canvas rendering, links and selectable text.                                                                           |
| [`@napi-rs/canvas`](https://github.com/Brooooooklyn/canvas)                                                                                          | MIT                       | Optional Node canvas backend pulled by PDF.js; it is not used or bundled by Zrimo's browser runtime.                                                    |
| [`image`](https://github.com/image-rs/image) and [`tiff`](https://github.com/image-rs/image-tiff)                                                    | MIT OR Apache-2.0 / MIT   | Multi-page TIFF decoding and PNG output inside the image WASM adapter.                                                                                  |
| [`wasm-bindgen`](https://github.com/wasm-bindgen/wasm-bindgen)                                                                                       | MIT OR Apache-2.0         | TypeScript/JavaScript bindings for Zrimo's Rust/WASM modules.                                                                                           |
| [`zip`](https://github.com/zip-rs/zip2)                                                                                                              | MIT                       | Bounded in-memory ZIP/OOXML package manipulation during legacy Office conversion.                                                                       |
| [Serde](https://github.com/serde-rs/serde), [`serde_json`](https://github.com/serde-rs/json) and [`thiserror`](https://github.com/dtolnay/thiserror) | MIT OR Apache-2.0         | Typed serialization and structured errors in the Rust core and worker boundary.                                                                         |
| [`core-js`](https://github.com/zloirock/core-js)                                                                                                     | MIT                       | Compatibility modules embedded in the PDF.js legacy browser build used by the supported browser matrix.                                                 |
| [Noto Sans](https://github.com/notofonts/noto-fonts) and [Noto Sans CJK](https://github.com/notofonts/noto-cjk)                                      | SIL Open Font License 1.1 | Self-hosted, lazy WOFF2 fallback packs for Latin/Cyrillic, Arabic, Indic, CJK, Japanese and Korean text.                                                |

Third-party components retain their own licenses. The package includes
`THIRD_PARTY_NOTICES.md`, its complete font notices and a generated SPDX SBOM
in the Zrimo repository.

## License

Zrimo's original source code and documentation are dual-licensed, at your
option, under the [MIT License](LICENSE-MIT) or the
[Apache License 2.0](LICENSE-APACHE). Copyright (c) 2026 Zrimo contributors.
