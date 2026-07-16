# @docs-viewer-wasm/viewer

Embeddable browser document viewer with an SSR-safe TypeScript facade and lazy Rust/WASM format adapters. No server conversion or telemetry is used.

```bash
npm install @docs-viewer-wasm/viewer
```

```ts
import { ViewerClient } from "@docs-viewer-wasm/viewer";
import "@docs-viewer-wasm/viewer/styles.css";

const client = ViewerClient.create({
  // Optional when your bundler handles package asset URLs normally:
  assetBaseUrl: new URL("/document-viewer/", location.href),
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

Supported formats are DOCX/DOCM, XLSX/XLSM, PPTX/PPTM/PPSX, DOC/XLS/PPT, PDF, PNG/JPEG/WebP/GIF/BMP/TIFF, SVG and CSV/TSV. Search, selection, pan/zoom, page/sheet navigation, thumbnails and original download are available through the same API. Images do not include OCR; encrypted documents and formula/macro execution are outside v1.

Subpath exports:

- `@docs-viewer-wasm/viewer` — complete public API and optional UI;
- `@docs-viewer-wasm/viewer/headless` — UI-free runtime/adapters;
- `@docs-viewer-wasm/viewer/worker` — custom worker adapter/RPC contracts;
- `@docs-viewer-wasm/viewer/styles.css`, `/fonts/*`, `/workers/*`, `/assets/*` — explicit assets.

The browser baseline is Safari 16.4+ and the latest two stable Chrome, Edge, Firefox and Safari releases. The package is MIT OR Apache-2.0; packaged Noto font subsets are OFL-1.1. See the repository documentation for API, compatibility, security, self-hosting and release artifacts.
