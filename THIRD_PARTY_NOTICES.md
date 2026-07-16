# Third-party notices

The release artifact contains or depends on the following principal components. The generated SPDX SBOM in `artifacts/sbom.spdx.json` is the complete machine-readable inventory for the pinned lockfiles.

- `@silurus/ooxml` — MIT; modern Office parsing/rendering.
- `office_oxide` — MIT OR Apache-2.0; legacy Office conversion.
- `pdf_oxide` — MIT OR Apache-2.0; PDF parsing/rendering.
- `image` and `tiff` Rust crates — MIT OR Apache-2.0; image encoding/decoding.
- `wasm-bindgen` — MIT OR Apache-2.0; browser bindings.
- Noto Sans and Noto Sans CJK subset fonts — SIL Open Font License 1.1. The font manifest, complete OFL text, pinned source commits and SHA-256 hashes are included in `dist/fonts/`.

No Microsoft proprietary font or copyleft runtime component is bundled. Transitive notices and license expressions are verified by `npm run licenses`.
