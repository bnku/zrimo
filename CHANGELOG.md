# Changelog

## 0.1.0 — 2026-07-17

- Renamed the product to Zrimo and moved the npm package to `@zrimo/viewer`.
- Renamed the optional UI integration surface to `.zrimo-ui`, `--zrimo-*`, and `data-zrimo-*`; the bundled fallback font family is now `Zrimo Noto`.
- Renamed the shared Rust crate to `zrimo-core` and the fuzz workspace package to `zrimo-fuzz`.
- Fixed DOCX selection geometry, PDF font/runtime compatibility, legacy DOC structured conversion, spreadsheet virtualization and multi-cell clipboard behavior.
- Added spreadsheet column resizing plus Shift/Ctrl/Cmd range selection.
- Added built-in loading indicators and complete Vanilla/React integration examples.
- Qualified structured Word 97–2003 DOC in Chromium, Firefox and WebKit and added it to the visual regression lane.
- Added the GitHub Pages landing/documentation/demo bundle and manual deployment workflow.
- Added reproducible repository, package-content, consumer, SBOM, license, vulnerability and size checks.

## 0.1.0-alpha.0 — 2026-07-16

- Added the complete v1 browser format pipeline for modern/legacy Office, PDF, raster/TIFF, SVG and CSV/TSV.
- Added headless and container APIs, virtualized viewport, navigation, pan/zoom/fit, Unicode search, text/cell selection and optional localized UI.
- Added lazy multilingual Noto font packs, worker cancellation/timeouts, bounded render scheduling and allocation limits.
- Added Chromium/Firefox/WebKit compatibility scenarios, browser fallbacks, SSIM goldens, fuzz targets, performance/size reports and license/vulnerability checks.
