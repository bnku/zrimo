# Test corpus and provenance

The repository stores a machine-readable manifest at `tests/corpus/manifest.json`, not third-party binary fixtures. `npm run corpus:fetch` downloads exact blobs into ignored `.cache/corpus/`, verifies SHA-256 before use, and fails closed on an upstream or checksum mismatch.

The qualification set contains Word 6/97/2000/2003 DOC cases (including tables, Unicode headers/footers, notes, lists and point/ranged comments), XLS, PPT, DOCX, XLSX, PPTX, PDF and additional renderer fixtures. Office files come from Apache POI commit `913c78891bd0cd20945b050c63abfb8c66c88009`; the PDF comes from Apache PDFBox commit `ddef86fcb1a5407035fdd1c8587832c3d1c761b9`. Both projects publish their test material under Apache-2.0. The manifest records repository, full commit, path, license, format, local name, and SHA-256 for every file.

## Expected outputs

- Word 97/2000/2003 DOC must parse source-backed formatting/stories/tables/lists/fields/notes/comments, convert qualified cases to a ZIP-signature DOCX buffer and reparse. Comment/numbering qualification checks package parts, references, relationships and content types; a separate public ranged-comment fixture verifies annotation CP bookmarks, strict `NilPICFAndBinData` classification, native conversion and browser rendering. Word 6 remains a typed unsupported revision. XLS/PPT must likewise convert to ZIP-signature OOXML and reparse as XLSX/PPTX.
- DOCX must produce at least one page and a non-empty 816×1056 canvas for the current sample; its Chromium screenshot is tracked as a visual baseline.
- PDF must report one page, produce a valid PNG at 72 DPI, and return a positioned text map containing characters.
- XLSX/PPTX are present for the production Office task; sheet/slide rendering assertions will be added there.

Run native qualification with `npm run test:qualification` and browser qualification with `npm run test:e2e`.

## Adding fixtures

Every new fixture needs a redistributable or fetchable source, a full immutable revision, an SPDX license compatible with the project policy, a SHA-256, and a short expected-behaviour assertion. Do not add customer documents, personal data, macros from unknown sources, or files with ambiguous licensing.

Generated fixtures should commit their deterministic generator and source text instead of an opaque binary where practical. Corrupt/adversarial files must state whether they are hand-generated, minimized fuzz outputs, or upstream security cases. Language fixtures must label script, language, expected text order, required fonts, and whether fonts are embedded.

The v1 release corpus target remains at least 20 representative files per format family plus corrupted, decompression-bomb, oversized-page/image, external-relationship, and active-SVG cases. The initial set proves plumbing only and is not a fidelity claim.

Corrective fidelity gates and the strict separation between committed/public
fixtures and private local inputs are documented in
[`regressions.md`](./regressions.md).
