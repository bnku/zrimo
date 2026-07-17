# Browser and format compatibility

## Supported browsers

The browser baseline is Safari 16.4+ and the latest two stable Chrome, Edge,
Firefox and Safari releases. Automated coverage exercises loading, navigation,
zoom, search, selection and cleanup in Chromium, Firefox and WebKit. WebKit is
useful compatibility coverage but cannot reproduce every Safari, operating
system and graphics-driver combination, so applications should validate their
own supported device matrix.

## Required and optional platform APIs

ES modules, WebAssembly, Worker, Canvas 2D, Blob, URL, TextEncoder/TextDecoder and Promise are required. `OffscreenCanvas`, `createImageBitmap`, `bitmaprenderer`, ResizeObserver, Fullscreen API, SIMD, threads, SharedArrayBuffer and cross-origin isolation are optional.

Automated fallback tests remove OffscreenCanvas, ResizeObserver and createImageBitmap, disable bitmaprenderer, and verify Canvas 2D rendering. Fullscreen absence leaves the viewer usable and the UI suppresses/handles the unavailable action. WASM modules are built without mandatory SIMD or threads.

## Format matrix

| Family             | Formats                              | Main path                                                                   | Known v1 limits                                                                                                                                                                         |
| ------------------ | ------------------------------------ | --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Modern Office      | DOCX/DOCM, XLSX/XLSM, PPTX/PPTM/PPSX | `@silurus/ooxml` parser/renderers                                           | Macros never run; formula cached values only; unsupported features warn                                                                                                                 |
| Legacy Office      | XLS, PPT                             | `office_oxide` WASM conversion to OOXML                                     | Conversion can reduce fidelity; password-protected files unsupported                                                                                                                    |
| Legacy Word Binary | Word 97–2003 DOC                     | Project-owned bounded Rust parser → DOCX in a module worker                 | Core runs, sections, fixed/auto-fit tables, lists, fields, notes and point/ranged comments are supported; shapes/OLE, old media and some advanced table/list variants remain incomplete |
| PDF                | PDF                                  | `pdfjs-dist` display API/module worker with local CMap/font/WASM/ICC assets | Password-protected files unsupported; XFA, editing and document scripts not executed                                                                                                    |
| Raster             | PNG, JPEG, WebP, GIF, BMP            | Browser decode                                                              | One browser-decoded frame for animation                                                                                                                                                 |
| TIFF               | TIFF, including multi-page           | Rust worker/WASM                                                            | Pixel and operation budgets apply                                                                                                                                                       |
| Vector             | SVG                                  | Sanitized browser decode                                                    | Scripts, foreign content and external resources removed                                                                                                                                 |
| Data               | CSV, TSV                             | Bounded parser, spreadsheet view                                            | Strings only; no formula evaluation                                                                                                                                                     |

Search/selection text quality depends on the source format exposing positioned text. Image-only documents do not perform OCR. Rendering is browser-side only; there is no server fallback or upload.

## Visual fidelity

Automated visual comparisons cover representative modern Office, legacy DOC,
PDF and image documents. Structural tests separately check page, sheet and
slide counts, text maps, hyperlinks and format-specific semantics. These tests
catch known regressions; they do not imply pixel-perfect support for every
producer or every construct. The format matrix above remains the source of
truth for unsupported and fidelity-degraded features.
