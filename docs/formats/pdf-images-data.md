# PDF, images, SVG, CSV and TSV

All non-Office v1 formats are built into the default `ViewerClient` registry.
PDF.js owns its parsing/render worker, TIFF and tabular parsing use project
module workers, and browser-native image decoding remains on the rendering side
because `CanvasImageSource` objects are browser resources.

## Capability matrix

| Input | Units | Backend | Selectable map | Important limits |
|---|---|---|---|---|
| PDF | pages | pinned `pdfjs-dist` display API + module worker | positioned text items with PDF.js font/transform metadata | no passwords; render pixel budget; scripting/XFA disabled |
| PNG, JPEG, WebP, GIF, BMP | one image | `createImageBitmap`, then `<img>` fallback | none | dimensions checked before decode; EXIF orientation requested from browser |
| TIFF | IFD pages | project `image-wasm` worker using pinned `tiff`/`image` crates | none | aggregate decoded page pixels bounded; supported Gray/GrayA/RGB/RGBA 8/16-bit and CMYK(A) 8-bit |
| SVG | one image | sanitized SVG, then browser decoder | none | active/external content removed before decode |
| CSV, TSV | one sheet | dependency-free parser worker | cell strings with 1-based row/column coordinates | input bytes plus default 1,000,000-cell cap |

All document/page indices are 0-based. Spreadsheet cell coordinates and `RenderViewport.sheetRange` are 1-based to match the Office sheet contract.

## PDF behavior

`PdfDocumentAdapter` lazily imports pinned `pdfjs-dist@6.1.200`, creates one
explicit module worker per document, and passes PDF bytes directly to
`getDocument`. A render request paints the `PDFPageProxy` directly into the
caller's canvas with zoom/DPR transform—there is no intermediate PNG, duplicate
Rust rasterizer, or page-image cache. The actual page pixel count is checked
against `maxDecodedPixels`; `RenderTask.cancel()` is wired to the viewer's
`AbortSignal`. Close cleans the document, loading task, page references, fonts,
and worker deterministically.

Canvas and selectable text come from the same PDF.js page and viewport.
`getTextContent()` items retain direction, generated font family, font size,
rotation, natural page extent, and logical UTF-16 offsets. The PDF-specific DOM
layer applies renderer widths through `scaleX`, while search highlight remains
inert. Safe link annotations are associated with intersecting runs; external
schemes use the common allowlist and internal destinations resolve to a page
when possible. Password failures return `encrypted-document`; malformed input
and render failures keep their typed contracts. Editing, form filling,
signature validation, XFA, and PDF JavaScript execution are not enabled.

Worker, packed Adobe CMaps, standard font programs, OpenJPEG/JBIG2/QCMS WASM and
ICC assets ship below `workers/` and `assets/pdfjs/`. URLs are resolved from
`assetBaseUrl` (or package-relative defaults), always with a trailing directory
slash. `useSystemFonts` is disabled so Base-14 output is deterministic and uses
the packaged standard-font data. No CDN or document-relative resource fetch is
performed.

```ts
const client = ViewerClient.create({ assetBaseUrl: "/viewer-assets/" });
const viewer = client.createViewer();
await viewer.load(pdfBytes, { fileName: "report.pdf" });
```

Self-host at least these package directories with the same relative layout:

- `workers/pdf.worker.min.mjs` (`worker-src` in CSP);
- `assets/pdfjs/cmaps/` and `assets/pdfjs/standard_fonts/` (`connect-src`);
- `assets/pdfjs/wasm/` and `assets/pdfjs/iccs/` (`connect-src`; WASM execution
  follows the browser's `script-src`/`wasm-unsafe-eval` policy).

## Raster and TIFF behavior

PNG/JPEG/WebP/GIF/BMP use the browser's decoder. `createImageBitmap` is requested with `imageOrientation: "from-image"`; exact codec and color-management behavior follows the browser. Canvas headless rendering captures one decoded frame. Animated GIF/WebP source bytes are retained, flagged in `DocumentInfo.warnings`, and remain available to the later basic UI's native image presentation path.

TIFF uses a separate lazy WASM artifact only when a TIFF is opened. Every IFD becomes a page and is normalized to compressed PNG inside the worker, so multi-page files use ordinary paginated rendering. Unsupported TIFF sample/color layouts return `invalid-file`; oversized aggregate pages return `resource-limit` before retained output grows without bound.

## SVG security policy

SVG is decoded only after sanitization. The sanitizer:

- removes `DOCTYPE`, `script`, `style`, `foreignObject`, iframe/object/embed/audio/video/canvas nodes;
- removes every `on*` event attribute;
- removes `href`, `xlink:href` and `src` values unless they are local fragment references;
- removes external `url(...)`, CSS expressions and imports.

This deliberately blocks external images, fonts, data URLs and CSS. A modified document emits `external-resource-blocked`. There is no default resource resolver or network fetch.

## CSV/TSV parsing rules

The worker recognizes UTF-8 (with or without BOM), UTF-16LE/BE BOM, then uses Windows-1252 only when input is invalid UTF-8 and emits `fidelity-degraded`. CSV delimiter detection compares comma, tab and semicolon outside quoted fields; `.tsv` always uses tab. RFC-style doubled quotes, quoted newlines, CRLF, empty cells and trailing empty fields are preserved as strings. No number, date or formula inference occurs.

The single sheet exposes used row/column bounds. `getTextMap` returns non-empty raw cell strings with cell coordinates, including Cyrillic, CJK, Arabic and Indic text. Rendering uses the shared sheet range and zoom contract; copying TSV from a selected range is implemented by the interaction layer in task 05.
