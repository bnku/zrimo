# Changelog

## 0.1.0-alpha.1 — Unreleased

- Renamed the product to Zrimo and moved the npm package to `@zrimo/viewer`.
- Renamed the optional UI integration surface to `.zrimo-ui`, `--zrimo-*`, and `data-zrimo-*`; the bundled fallback font family is now `Zrimo Noto`.
- Renamed the shared Rust crate to `zrimo-core` and the fuzz workspace package to `zrimo-fuzz`.
- Kept the quarantined pre-Zrimo `@docs-viewer-wasm/viewer@0.1.0-alpha.0` tarball immutable for historical diagnosis only.

## 0.1.0-alpha.0 — 2026-07-16

- Added the complete v1 browser format pipeline for modern/legacy Office, PDF, raster/TIFF, SVG and CSV/TSV.
- Added headless and container APIs, virtualized viewport, navigation, pan/zoom/fit, Unicode search, text/cell selection and optional localized UI.
- Added lazy multilingual Noto font packs, worker cancellation/timeouts, bounded render scheduling and allocation limits.
- Added Chromium/Firefox/WebKit compatibility scenarios, browser fallbacks, SSIM goldens, fuzz targets, performance/size reports and license/vulnerability gates.
- Initial release candidate has no prior public visual, performance or bundle regression baseline.
