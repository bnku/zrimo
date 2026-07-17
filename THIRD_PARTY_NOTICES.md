# Third-party notices

The release artifact contains or depends on the following principal components. The generated SPDX SBOM in `artifacts/sbom.spdx.json` is the complete machine-readable inventory for the pinned lockfiles.

- `@silurus/ooxml` — MIT; modern Office parsing/rendering.
- `office_oxide` — MIT OR Apache-2.0; legacy Office conversion.
- `pdfjs-dist` / Mozilla PDF.js — Apache-2.0; browser PDF parsing,
  font/CMap handling, canvas rendering, and text extraction. The packaged
  standard-font, ICC, CMap, OpenJPEG, JBIG2, and QCMS assets retain the license
  files distributed with PDF.js under `dist/assets/pdfjs/`.
- `core-js` compatibility modules embedded in the PDF.js legacy browser build —
  MIT; polyfills required by the supported browser matrix, including the PDF
  worker realm.
- `image` and `tiff` Rust crates — MIT OR Apache-2.0; image encoding/decoding.
- `wasm-bindgen` — MIT OR Apache-2.0; browser bindings.
- Noto Sans and Noto Sans CJK subset fonts — SIL Open Font License 1.1. The font manifest, complete OFL text, pinned source commits and SHA-256 hashes are included in `dist/fonts/`.

No Microsoft proprietary font or copyleft runtime component is bundled. Transitive notices and license expressions are verified by `npm run licenses`.
