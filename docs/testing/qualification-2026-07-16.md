# Release-candidate qualification report — 2026-07-16

## Environment

- Linux 6.12.73 x86_64
- AMD Ryzen 9 5950X, 16 cores / 32 threads, 94 GiB RAM
- Node 24.18.0, npm 11.16.0
- Rust 1.94.1, `wasm32-unknown-unknown`
- Chromium 145.0.7632.109

Numbers are regression baselines on an otherwise non-isolated development host. Real-device/browser release smoke remains a separate checklist item.

## Commands and results

| Gate | Command | Result |
|---|---|---|
| TypeScript/Rust fast suite | `npm run check` | 65 TypeScript tests, Rust tests/clippy/format, typecheck, and license inventory pass |
| Native candidate qualification | `npm run test:qualification` | DOC/XLS/PPT conversion and PDF bitmap/text tests pass |
| Browser qualification | `npm run test:e2e` | 21 Chromium flows cover SSR, viewport/UI/selection, multilingual fonts, formats, fallbacks, lifecycle, performance and four SSIM families |
| Browser matrix | `npm run test:e2e:matrix` | Chromium, Firefox and WebKit pass the public lifecycle plus optional-API fallback scenario |
| Fuzzing | `npm run fuzz:js`; `npm run fuzz:rust` | 2,000 JS mutations and four 10-second libFuzzer targets pass without crash |
| Vulnerabilities | `npm run audit:vulnerabilities` | 0 npm production and 0 Cargo vulnerabilities |
| WASM release build | `npm run build:wasm-bindings` | Legacy Office, PDF and TIFF project bindings generated and Binaryen-optimized |
| Size inventory | `npm run report:size` | Full base code+WASM is 9.97 MiB raw / 4.30 MiB gzip / 3.26 MiB Brotli |
| Packed consumers | `npm run test:pack` | 111-file alpha tarball passes content/integrity, SSR, strict TS, esbuild, Vite, webpack and Next.js production builds |
| Supply chain | `npm run report:sbom` | SPDX 2.3 SBOM and SHA-256/npm integrity artifacts generated |

Representative warm browser timings from the final run were 104.2 ms for DOCX load plus first render, 17.2 ms for a 6.5 KiB DOC conversion, and 65.1 ms for parsing/rasterizing/extracting the 3.5 KiB one-page PDF. The DOC conversion produced 2,203 bytes of valid OOXML; PDF produced a 16,140-byte PNG and nine positioned characters. These values are logged as `QUALIFICATION_METRIC` records by Playwright and are regression observations, not public latency guarantees.

## WASM size baseline

| Lazy artifact | Raw | Gzip | Brotli |
|---|---:|---:|---:|
| OOXML DOCX parser | 0.85 MiB | 0.37 MiB | 0.28 MiB |
| OOXML XLSX parser | 0.78 MiB | 0.35 MiB | 0.26 MiB |
| OOXML PPTX parser | 0.82 MiB | 0.35 MiB | 0.27 MiB |
| Legacy Office binding | 1.08 MiB | 0.52 MiB | 0.41 MiB |
| PDF binding | 5.69 MiB | 2.44 MiB | 1.82 MiB |
| TIFF image binding | 0.52 MiB | 0.20 MiB | 0.16 MiB |
| Total | 9.74 MiB | 4.24 MiB | 3.21 MiB |

This table covers WASM only. It excludes JavaScript renderers, CSS, workers, source maps, and optional font packs; the release gate measures the complete packed npm artifact. Lazy loading means a single document normally transfers only its relevant family.

The optional OFL-1.1 WOFF2 directory is 10,958,004 bytes across 14 script packs. It is excluded from the base transfer budget and fetched by encountered script; Arabic plus Devanagari, for example, requests only those two files. `manifest.json` records each pack's pinned source, byte size and SHA-256.

## Visual and memory baseline

The four pinned renderer-family goldens cover modern Office, legacy Office, PDF and image. The final global luminance SSIM was `1.0` for each family, above the respective `0.94`, `0.90`, `0.97` and `0.97` gates. The DOCX canvas also has a deterministic pixel snapshot checked on every Chromium run.

No reliable cross-browser peak-memory API is available under the baseline non-isolated origin. Memory safety evidence therefore comes from bounded page/text/pixel/cache allocations, virtualized DOM, repeated lifecycle teardown, worker timeout/termination tests and successful completion across Chromium, Firefox and WebKit. Device-level heap profiling remains part of real-browser release smoke before 1.0.

## Decision

The `0.1.0-alpha.0` artifact is accepted for authorized alpha publication. There is no unknown backend choice, required copyleft component, known production vulnerability or failing automated gate. Promotion to 1.0 remains conditional on real-browser/device smoke, corpus expansion and alpha/beta feedback listed in the release checklist.
