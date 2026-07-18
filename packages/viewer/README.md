# @zrimo/viewer

**Any document. One canvas.**

[Website](https://bnku.github.io/zrimo/) ·
[Live React demo](https://bnku.github.io/zrimo/demo/) ·
[Documentation](https://bnku.github.io/zrimo/getting-started) ·
[GitHub](https://github.com/bnku/zrimo)

Zrimo renders Office documents, PDFs, images and structured data directly
inside your web application. Files stay in the browser: there is no conversion
server, upload step or telemetry.

## Why Zrimo?

- **One viewer for common business formats.** Open DOCX, XLSX, PPTX, legacy
  Office, PDF, images, SVG, CSV and TSV through one TypeScript API.
- **Use our interface or bring your own.** Mount the built-in toolbar, hide it
  behind React controls, or render headlessly to your own canvas.
- **Real document interactions.** Pan, zoom, fit, search, selectable text,
  thumbnails, sheet navigation, multi-cell selection and TSV clipboard output.
- **International documents.** Latin, Cyrillic, CJK, Arabic and supported Indic
  scripts use application fonts, system fonts or lazy self-hosted fallbacks.
- **Browser-private by design.** Parsers, renderers and legacy converters run
  locally; format-specific Rust/WASM modules load only when needed.

## Install

```bash
npm install @zrimo/viewer
npx zrimo-copy-assets public/zrimo
```

The second command copies workers, WASM modules and optional font assets into
your application's public directory.

## Quick start

```ts
import { ViewerClient } from "@zrimo/viewer";
import "@zrimo/viewer/styles.css";

const client = ViewerClient.create({
  assetBaseUrl: new URL("/zrimo/", location.href),
});

const viewer = client.createViewer({
  container: document.querySelector("#viewer")!,
  ui: true,
  fit: "width",
});

await viewer.load(file, { fileName: file.name });

// When the host component unmounts:
await viewer.destroy();
await client.destroy();
```

Give the host element an explicit size:

```css
#viewer {
  width: 100%;
  height: 100%;
  min-height: 320px;
}
```

See the [installation guide](https://bnku.github.io/zrimo/getting-started) for
framework setup, asset paths and production MIME configuration.

## Choose your integration

| Approach    | Configuration                            | Best for                                              |
| ----------- | ---------------------------------------- | ----------------------------------------------------- |
| Built-in UI | `createViewer({ container, ui: true })`  | A complete viewer with minimal host code              |
| Custom UI   | `createViewer({ container, ui: false })` | React/Vue/Svelte controls around the managed viewport |
| Headless    | `@zrimo/viewer/headless`                 | Custom canvas rendering and application-owned layout  |

The [live React demo](https://bnku.github.io/zrimo/demo/) shows the built-in UI,
a fully custom React toolbar and the headless API side by side.

## Supported formats

| Family               | Formats                                              |
| -------------------- | ---------------------------------------------------- |
| Modern Office        | DOCX, DOCM, XLSX, XLSM, PPTX, PPTM, PPSX             |
| Legacy Office        | Word 97–2003 DOC, BIFF8 XLS, PPT                     |
| Documents and images | PDF, PNG, JPEG, WebP, GIF, BMP, multi-page TIFF, SVG |
| Structured data      | CSV, TSV                                             |

Macros and document scripts never execute. Spreadsheet formulas display their
stored cached values. Images are rendered without OCR. Password-protected
documents are not currently supported, and complex legacy Office objects may
render with reduced fidelity. The maintained details live in the
[compatibility guide](https://bnku.github.io/zrimo/compatibility).

## Public entry points

- `@zrimo/viewer` — complete API and optional built-in UI;
- `@zrimo/viewer/headless` — UI-free runtime and adapters;
- `@zrimo/viewer/worker` — worker adapter and RPC contracts;
- `@zrimo/viewer/styles.css` — built-in UI styles;
- `@zrimo/viewer/assets/*`, `/workers/*`, `/fonts/*` — explicit runtime assets.

Start with the [API reference](https://bnku.github.io/zrimo/api/reference),
[UI guide](https://bnku.github.io/zrimo/ui) or
[framework integrations](https://bnku.github.io/zrimo/integrations).

## Browser support

Zrimo targets Safari 16.4+ and the latest two stable Chrome, Edge, Firefox and
Safari releases. Automated browser coverage runs in Chromium, Firefox and
WebKit.

## Built on open source

Zrimo exists thanks to the maintainers and contributors behind these projects:

| Project                                                                                                                                              | License                   | Used for                                                              |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------- | --------------------------------------------------------------------- |
| [Office Open XML Viewer (`@silurus/ooxml`)](https://github.com/yukiyokotani/office-open-xml-viewer)                                                  | MIT                       | Rust/WASM parsing and Canvas rendering for DOCX, XLSX and PPTX        |
| [`office_oxide`](https://github.com/yfedoseev/office_oxide)                                                                                          | MIT OR Apache-2.0         | Compound files, shared Office utilities and legacy XLS/PPT conversion |
| [Mozilla PDF.js](https://github.com/mozilla/pdf.js)                                                                                                  | Apache-2.0                | PDF parsing, fonts, Canvas rendering, links and selectable text       |
| [`@napi-rs/canvas`](https://github.com/Brooooooklyn/canvas)                                                                                          | MIT                       | Optional PDF.js Node dependency; not bundled into the browser runtime |
| [`image`](https://github.com/image-rs/image) and [`tiff`](https://github.com/image-rs/image-tiff)                                                    | MIT OR Apache-2.0 / MIT   | Multi-page TIFF decoding and image output                             |
| [`wasm-bindgen`](https://github.com/wasm-bindgen/wasm-bindgen)                                                                                       | MIT OR Apache-2.0         | JavaScript and TypeScript bindings for Rust/WASM modules              |
| [`zip`](https://github.com/zip-rs/zip2)                                                                                                              | MIT                       | Bounded in-memory ZIP and OOXML package handling                      |
| [Serde](https://github.com/serde-rs/serde), [`serde_json`](https://github.com/serde-rs/json) and [`thiserror`](https://github.com/dtolnay/thiserror) | MIT OR Apache-2.0         | Serialization and structured Rust errors                              |
| [`core-js`](https://github.com/zloirock/core-js)                                                                                                     | MIT                       | Compatibility modules used by the PDF.js legacy browser build         |
| [Noto Sans](https://github.com/notofonts/noto-fonts) and [Noto Sans CJK](https://github.com/notofonts/noto-cjk)                                      | SIL Open Font License 1.1 | Lazy self-hosted multilingual fallback fonts                          |

Third-party components retain their own licenses. The npm package includes
`THIRD_PARTY_NOTICES.md` and its font notices. The complete pinned inventory is
available as the repository's
[generated SPDX SBOM](https://github.com/bnku/zrimo/blob/main/artifacts/sbom.spdx.json).

## License

Zrimo's original source code and documentation are dual-licensed, at your
option, under the [MIT License](LICENSE-MIT) or the
[Apache License 2.0](LICENSE-APACHE). Copyright (c) 2026 Zrimo contributors.
