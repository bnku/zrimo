# Qualified dependencies

Qualification date: 2026-07-16. Runtime versions are exact pins in `package-lock.json` and `Cargo.lock`; build-tool versions are pinned in `package.json` and `rust-toolchain.toml`.

| Component | Pin | License | Enabled scope | Qualification result |
|---|---:|---|---|---|
| `@silurus/ooxml` | `0.72.2` | MIT | DOCX/XLSX/PPTX parser WASM and canvas renderers | DOCX/XLSX/PPTX open through the package default adapter in Chromium; DOCX has a repeatable screenshot baseline |
| `office_oxide` | `0.1.6` | MIT OR Apache-2.0 | Default features disabled; library API wrapped by our WASM crate | DOC/XLS/PPT fixtures convert in a browser module worker and the generated OOXML opens through the same package adapters |
| `pdf_oxide` | `0.3.74` | MIT OR Apache-2.0 | Default features disabled; `rendering` only | Native and Chromium smoke tests produced PNG bytes and positioned page text |
| `image` / `tiff` | `0.25.10` / `0.11.3` | MIT OR Apache-2.0 | TIFF fallback only; `image` enables PNG output, browser handles other raster formats | Multi-page TIFF unit and Chromium worker decode pass with aggregate pixel enforcement |
| `wasm-bindgen` / CLI | `0.2.126` | MIT OR Apache-2.0 | Browser glue for our Rust crates | Version-matched CLI produces `web` bindings; installed locally under `.tools/` |
| Binaryen | `131.0.0` | Apache-2.0 | Build-only WASM optimization | `-O4`/shrink-level 2 equivalent reduces generated browser binaries |
| TypeScript | `7.0.2` | Apache-2.0 | Build and declarations | Strict ESM declarations and SSR-safe package import pass |
| Playwright | `1.61.1` | Apache-2.0 | Browser tests only | System Chromium smoke and golden screenshot pass |
| esbuild | `0.28.1` | MIT | Example bundling/server only | Vanilla integration bundle and local test server pass |
| Noto Sans / Noto Sans CJK | commits `ffebf8c…` / `f8d1575…` | OFL-1.1 | 14 optional script-range WOFF2 fallback assets | SHA-256 manifest, full language-corpus load and no-third-party-network Chromium tests pass |

The selected `office_oxide` release does not expose bytes-out in its stock JavaScript binding, but no fork is needed: the Rust API publicly exposes `create_from_ir_to_writer`. `crates/legacy-office-wasm` is therefore a small project-owned binding, not an upstream patch. Its generated web binding is loaded only inside `legacy-converter-worker.js`; conversion uses bytes-in/bytes-out transferables and no filesystem or server.

`pdf_oxide` is intentionally built without its default ICC and legacy-crypto set and without its broad `wasm` feature, which would enable unrelated signatures and barcode functionality. Our binding calls the public Rust parser, renderer, and positioned text APIs directly.

## License policy

`npm run licenses` inventories every package-lock and Cargo metadata entry. Allowed code licenses are 0BSD, MIT/MIT-0, Apache-2.0, BSD-2-Clause, BSD-3-Clause, BSL-1.0, ISC, CC0-1.0, Unicode-3.0, Unlicense, and Zlib. SPDX `OR` expressions pass when at least one complete alternative is allowed; a forbidden GPL/LGPL/AGPL/MPL/EPL/CDDL/SSPL alternative is never selected.

The current 2026-07-16 baseline passes for 61 npm lock entries and 193 Cargo packages. Vitest/Vite was removed during qualification because Vite's development tree introduced MPL-2.0 `lightningcss`; unit tests now use the zero-dependency Node test runner. `deny.toml` mirrors the Cargo allowlist for environments with `cargo-deny` installed.

Font assets additionally allow OFL-1.1. Their pinned provenance, modified family name, checksums, license text and notices ship in `packages/viewer/fonts`; they are not JavaScript/Cargo dependencies and are audited from that manifest.

## Requalification rule

Changing any runtime version or feature set requires all of the following in the same change: a clean license inventory, `wasm32-unknown-unknown` release build, qualification corpus run, Chromium smoke run, size report, and an update to this document. Git dependencies must be pinned to a full commit; none are currently used.
