# Zrimo

**Any document. One canvas.**

Zrimo is a framework-agnostic, browser-side document viewer with a TypeScript API and lazily loaded Rust/WASM adapters. It renders modern Office, qualified Word 97–2003 DOC and legacy XLS/PPT, PDF, images, SVG and delimited data without uploading documents to a conversion service.

## Supported formats

- DOCX/DOCM, XLSX/XLSM, PPTX/PPTM/PPSX
- Word 97–2003 DOC through the project-owned bounded Word Binary parser and in-memory DOCX serialization
- XLS and PPT through in-memory legacy → OOXML normalization
- PDF
- PNG, JPEG, WebP, GIF, BMP and multi-page TIFF
- SVG, CSV and TSV

The viewer supports Latin/Cyrillic, CJK, Arabic-script and the agreed Indic scripts through application/system fonts plus lazy self-hosted Noto WOFF2 packs. Macros and document scripts never execute; spreadsheet formulas display stored cached values.

## Install and quick start

```bash
npm install @zrimo/viewer
npx zrimo-copy-assets public/vendor/zrimo
```

```ts
import { ViewerClient } from "@zrimo/viewer";
import "@zrimo/viewer/styles.css";

const client = ViewerClient.create({
  assetBaseUrl: new URL("/vendor/zrimo/", location.href),
});
const viewer = client.createViewer({
  container: document.querySelector("#viewer")!,
  ui: true,
  fit: "width",
});

await viewer.load(file, { fileName: file.name });
// Later:
await viewer.destroy();
await client.destroy();
```

Use `@zrimo/viewer/headless` for UI-free integration and `@zrimo/viewer/worker` for custom adapter infrastructure. The [getting-started guide](docs/getting-started.md) covers CDN/self-host layouts and MIME requirements.

## Browser and privacy baseline

The target is Safari 16.4+ and the latest two stable Chrome, Edge, Firefox and Safari versions. Automated lifecycle/fallback scenarios run in Playwright Chromium, Firefox and WebKit; real-browser smoke is mandatory before a stable release.

Documents remain in the browser. There is no telemetry, server conversion or implicit external relationship fetch. Network traffic is limited to an explicit URL source and package/application-owned assets selected by the font and `assetBaseUrl` policies.

## Develop and verify

Prerequisites are Node.js 22.13+ or 24+, npm 11, Rust 1.94.1 with `wasm32-unknown-unknown`, Chromium, and Rust nightly/cargo-fuzz only for the fuzz gate.

```bash
npm ci
npm run build
npm run check
npm run test:qualification
npm run test:e2e
npm run test:e2e:matrix
npm run test:pack
```

Release/security gates additionally include `npm run fuzz:js`, `npm run fuzz:rust`, `npm run audit:vulnerabilities`, `npm run report:size`, and `npm run report:sbom`.

Run either development example from the workspace root:

```bash
npm run dev --workspace @zrimo/example-vanilla
npm run dev --workspace @zrimo/example-react
```

Build and inspect the complete site — landing, documentation and the production
React demo — locally:

```bash
npm run pages:build
npm run pages:preview
# open http://127.0.0.1:4174
```

## Documentation

- [API reference](docs/api/reference.md) and [headless guide](docs/api/headless.md)
- [UI](docs/ui.md), [fonts/self-hosting](docs/fonts.md), and [framework integrations](docs/integrations.md)
- [formats/browser compatibility](docs/compatibility.md) and [troubleshooting](docs/troubleshooting.md)
- [security model](docs/security.md) and [performance guidance](docs/performance.md)
- [architecture](docs/architecture.md)
- [contribution guide](CONTRIBUTING.md)

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
| [Heroicons](https://github.com/tailwindlabs/heroicons)                                                                                               | MIT                       | Consistent outline icons on the project landing page.                                                                                                   |

Third-party components retain their own licenses. See
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) for distribution notices and
the generated [`artifacts/sbom.spdx.json`](artifacts/sbom.spdx.json) for the
complete pinned transitive inventory.

## License

Zrimo's original source code and documentation are dual-licensed, at your
option, under the [MIT License](LICENSE-MIT) or the
[Apache License 2.0](LICENSE-APACHE). Copyright (c) 2026 Zrimo contributors.

Unless explicitly stated otherwise, any contribution intentionally submitted
for inclusion in Zrimo is provided under the same `MIT OR Apache-2.0` terms,
without additional restrictions.
