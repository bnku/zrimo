# @zrimo/viewer

**Any document. One canvas.**

Zrimo is an embeddable browser document viewer with an SSR-safe TypeScript facade and lazy Rust/WASM format adapters. No server conversion or telemetry is used.

```bash
npm install @zrimo/viewer
```

```ts
import { ViewerClient } from "@zrimo/viewer";
import "@zrimo/viewer/styles.css";

const client = ViewerClient.create({
  // Optional when your bundler handles package asset URLs normally:
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

Supported render formats are DOCX/DOCM, XLSX/XLSM, PPTX/PPTM/PPSX, Word 97–2003 DOC, legacy XLS/PPT, PDF, PNG/JPEG/WebP/GIF/BMP/TIFF, SVG and CSV/TSV. DOC is parsed by a bounded project-owned Rust backend, serialized in memory to DOCX, and then uses the same page renderer and selectable text layer as DOCX. Source comment authors, bodies and point anchors are preserved; range anchors are still fidelity-degraded. Other advanced Word Binary features such as Word 6, OLE objects, complex floating shapes, complete list semantics and some image/table variants remain unsupported or fidelity-degraded. Search, selection, pan/zoom, page/sheet navigation, thumbnails and original download are available through the same API. Images do not include OCR; encrypted documents and formula/macro execution are outside v1.

Sheets use a dedicated virtual spreadsheet surface: the scrollbar covers the
complete used range and the visible trailing blank rows/columns, while only one
viewport-sized canvas is rendered. Custom and hidden band sizes, frozen panes,
sparse far-away cells, zoom `0.1…8`, cell selection, and TSV copy through
`copySelection()` or Ctrl/Cmd+C do not depend on an A4 page box. The vanilla example's
`?large-sheet=1` mode creates a synthetic 250×100 CSV to demonstrate deep
horizontal and vertical scrolling without shipping a fixture.

Subpath exports:

- `@zrimo/viewer` — complete public API and optional UI;
- `@zrimo/viewer/headless` — UI-free runtime/adapters;
- `@zrimo/viewer/worker` — custom worker adapter/RPC contracts;
- `@zrimo/viewer/styles.css`, `/fonts/*`, `/workers/*`, `/assets/*` — explicit assets.

The browser baseline is Safari 16.4+ and the latest two stable Chrome, Edge, Firefox and Safari releases. The package is MIT OR Apache-2.0; packaged Noto font subsets are OFL-1.1. See the repository documentation for API, compatibility, security, self-hosting and release artifacts.
