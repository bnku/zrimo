# Release qualification — 0.1.0

This is the sanitized local qualification result for `@zrimo/viewer@0.1.0` on
2026-07-17. It contains no private input names, hashes, document text,
screenshots, or derived documents. No npm publish, Git tag, GitHub Release, or
Pages deployment was performed.

## Result

The stable `0.1.0` candidate passes the repository, runtime, browser, fidelity,
security, packaging, and documentation gates. `release-status.json` and the
pack report both mark it as ready.

| Gate | Result |
| --- | --- |
| Repository payload | 254 candidate files; no ignored, private, generated, or oversized unintended files |
| TypeScript and Rust | Typecheck, format, clippy; 69 viewer and 68 Rust unit tests pass |
| Public qualification corpus | 7 legacy DOC/XLS/PPT tests pass |
| Chromium E2E | 35/35 pass, including modern Office, legacy DOC, PDF, images, fonts, selection, spreadsheets, lifecycle, and security |
| Browser matrix | 40/40 pass without skips on Chromium, Chromium DPR2, Firefox, and WebKit |
| Fidelity | Modern Office, legacy DOC, PDF, and image goldens pass; DOC SSIM 1.0 at a 0.94 threshold |
| Fuzzing | 2,000 JS mutations and five Rust fuzz targets; zero crashes or failures |
| Dependencies | 5 npm runtime packages, 61 Cargo packages, and 14 font assets pass the license policy |
| Vulnerabilities | 0 npm production and 0 Cargo advisories |
| Size | Base 3,799,154 bytes Brotli; 14,759,868 bytes with all optional fonts; pass |
| Package | 308 files, 14,221,187-byte tarball, recursive content scan clean, private inputs excluded |
| Consumer matrix | Plain ESM/SSR, strict TypeScript, esbuild, Vite, webpack, and Next.js/webpack pass |
| Runtime assets | Installed package CLI copies WASM, PDF worker, and font manifest successfully |
| GitHub Pages | Landing, documentation, and React demo build and pass the `/zrimo/` subpath smoke test |
| Registry simulation | `npm publish --dry-run --access public` accepts `@zrimo/viewer@0.1.0` |

Machine-readable evidence is stored in `artifacts/`: browser capabilities,
fidelity, fuzz, performance, SPDX SBOM, size, vulnerability, and pack reports.
The exact release tarball and checksum are generated only in ignored `.cache`
until the authorized publication workflow stages them.
