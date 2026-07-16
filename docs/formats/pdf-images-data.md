# PDF, images, SVG, CSV and TSV

All non-Office v1 formats are built into the default `ViewerClient` registry. Binary parsing/rasterization and tabular parsing use dedicated module workers; browser-native image decoding remains on the rendering side because `CanvasImageSource` objects are browser resources.

## Capability matrix

| Input | Units | Backend | Selectable map | Important limits |
|---|---|---|---|---|
| PDF | pages | `pdf_oxide` rendering-only WASM worker | positioned Unicode characters | no passwords; per-page and cache pixel budget |
| PNG, JPEG, WebP, GIF, BMP | one image | `createImageBitmap`, then `<img>` fallback | none | dimensions checked before decode; EXIF orientation requested from browser |
| TIFF | IFD pages | project `image-wasm` worker using pinned `tiff`/`image` crates | none | aggregate decoded page pixels bounded; supported Gray/GrayA/RGB/RGBA 8/16-bit and CMYK(A) 8-bit |
| SVG | one image | sanitized SVG, then browser decoder | none | active/external content removed before decode |
| CSV, TSV | one sheet | dependency-free parser worker | cell strings with 1-based row/column coordinates | input bytes plus default 1,000,000-cell cap |

All document/page indices are 0-based. Spreadsheet cell coordinates and `RenderViewport.sheetRange` are 1-based to match the Office sheet contract.

## PDF behavior

`PdfDocumentAdapter` keeps a persistent worker and a parsed `PdfViewerDocument`. A render request selects a DPI from zoom and device pixel ratio, receives PNG bytes through a transferable buffer, enforces `maxDecodedPixels`, and inserts the compressed page into an LRU cache whose aggregate pixels use the same budget. Close terminates the worker and clears the cache.

`getTextMap` maps the backend's per-character Unicode and PDF-point bounding boxes into common `TextRun` records. This preserves glyph/range geometry for search and selection in task 05. The current minimal binding exposes renderer-visible page content; editing, form filling, signature validation and annotation editing are not implemented. Files with `/Encrypt` or an encryption/password backend error return `encrypted-document`.

```ts
const client = ViewerClient.create({ assetBaseUrl: "/viewer-assets/" });
const viewer = client.createViewer();
await viewer.load(pdfBytes, { fileName: "report.pdf" });
```

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
