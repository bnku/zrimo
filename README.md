# docs-viewer-wasm

Framework-agnostic, browser-side document viewer with a TypeScript API and lazily loaded Rust/WASM adapters. It renders modern Office, qualified Word 97–2003 DOC and legacy XLS/PPT, PDF, images, SVG and delimited data without uploading documents to a conversion service.

## Supported formats

- DOCX/DOCM, XLSX/XLSM, PPTX/PPTM/PPSX
- Word 97–2003 DOC through the project-owned bounded Word Binary parser and in-memory DOCX serialization
- XLS and PPT through in-memory legacy → OOXML normalization
- PDF
- PNG, JPEG, WebP, GIF, BMP and multi-page TIFF
- SVG, CSV and TSV

The viewer supports Latin/Cyrillic, CJK, Arabic-script and the agreed Indic scripts through application/system fonts plus lazy self-hosted Noto WOFF2 packs. Macros and document scripts never execute; spreadsheet formulas display stored cached values.

## Install and quick start

The existing `@docs-viewer-wasm/viewer@0.1.0-alpha.0` artifact is quarantined
after fidelity regressions were found in DOCX selection, PDF fonts, legacy DOC
layout, and XLSX scrolling. It must not be promoted or treated as a qualified
release. For historical local diagnosis only, the artifact remains at:

```bash
npm install ./artifacts/docs-viewer-wasm-viewer-0.1.0-alpha.0.tgz
```

After publication, the registry command will be:

```bash
npm install @docs-viewer-wasm/viewer
```

```ts
import { ViewerClient } from "@docs-viewer-wasm/viewer";
import "@docs-viewer-wasm/viewer/styles.css";

const client = ViewerClient.create();
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

Use `@docs-viewer-wasm/viewer/headless` for UI-free integration and `@docs-viewer-wasm/viewer/worker` for custom adapter infrastructure. `assetBaseUrl` supports explicit CDN/self-host layouts when a bundler does not emit package assets automatically.

## Browser and privacy baseline

The target is Safari 16.4+ and the latest two stable Chrome, Edge, Firefox and Safari versions. Automated lifecycle/fallback scenarios run in Playwright Chromium, Firefox and WebKit; real-browser smoke is mandatory before a stable release.

Documents remain in the browser. There is no telemetry, server conversion or implicit external relationship fetch. Network traffic is limited to an explicit URL source and package/application-owned assets selected by the font and `assetBaseUrl` policies.

## Develop and verify

Prerequisites are Node.js 22+, npm 11, Rust 1.94.1 with `wasm32-unknown-unknown`, Chromium, and Rust nightly/cargo-fuzz only for the fuzz gate.

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
npm run dev --workspace @docs-viewer-wasm/example-vanilla
npm run dev --workspace @docs-viewer-wasm/example-react
```

## Documentation

- [API reference](docs/api/reference.md) and [headless guide](docs/api/headless.md)
- [UI](docs/ui.md), [fonts/self-hosting](docs/fonts.md), and [framework integrations](docs/integrations.md)
- [formats/browser compatibility](docs/compatibility.md) and [troubleshooting](docs/troubleshooting.md)
- [security model](docs/security.md), [performance budgets](docs/performance.md), and [release checklist](docs/release-checklist.md)
- [architecture](docs/architecture.md) and [implementation roadmap](docs/universal-document-viewer/00-roadmap.md)

The project is dual-licensed MIT OR Apache-2.0. Third-party components remain under their listed permissive/OFL licenses; see `THIRD_PARTY_NOTICES.md` and the generated SPDX SBOM.
