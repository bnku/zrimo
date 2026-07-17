# Office formats

The built-in `OfficeDocumentAdapter` is registered by `ViewerClient` and routes modern OOXML and legacy binary Office files through one document model. Detection is content-first; file names and MIME types preserve macro/slideshow subtypes only when they agree with the detected family.

## Format matrix

| Input            | Internal path                     | Unit  | Rendering and text                                                                  |
| ---------------- | --------------------------------- | ----- | ----------------------------------------------------------------------------------- |
| DOCX, DOCM       | `@silurus/ooxml` DOCX engine      | page  | Section-aware canvas pages and positioned text runs                                 |
| XLSX, XLSM       | `@silurus/ooxml` XLSX engine      | sheet | Sheet viewport, merged ranges, frozen panes, cell coordinates and positioned text   |
| PPTX, PPTM, PPSX | `@silurus/ooxml` PPTX engine      | slide | Master/layout-aware canvas slides and positioned text runs                          |
| Word 97–2003 DOC | `legacy-doc` WASM → DOCX engine   | page  | Source-backed runs, paragraphs, sections, tables, lists, fields, notes and comments |
| XLS              | `office_oxide` WASM → XLSX engine | sheet | Same navigation/text semantics as XLSX after in-memory normalization                |
| PPT              | `office_oxide` WASM → PPTX engine | slide | Same navigation/text semantics as PPTX after in-memory normalization                |

DOCX canvas pages and selectable DOM are driven by the same
`DocxTextRunInfo` geometry. The adapter retains CSS font shorthand/size, letter
spacing, page transform, tate-chu-yoko metadata, hyperlink target, coordinate
extent, and logical UTF-16 offsets. The viewport delegates selection and find
geometry to the pinned `@silurus/ooxml` overlay builders; zoom scales the entire
natural coordinate layer rather than re-estimating individual glyph boxes.

Modern OOXML parsing occurs in the backend's module worker. DOC/XLS/PPT conversion occurs in the package's `legacy-converter-worker.js`: input and generated OOXML use transferable buffers, never a Blob URL, server request, native executable, or temporary file. DOC uses the project-owned bounded `legacy-doc` parser instead of the heuristic upstream DOC projection. Generated OOXML is an internal derived artifact; only the original input remains available through `getOriginalBytes()` and all parser/converter state is released on `close()`/`destroy()`.

The package distribution contains the converter worker and `assets/legacy/index.js`/`index_bg.wasm`. With a self-hosted asset directory, set `assetBaseUrl`; the expected paths are `workers/legacy-converter-worker.js` and `assets/legacy/index.js`. `legacy.workerUrl` and `legacy.moduleUrl` can be overridden when constructing `OfficeDocumentAdapter`.

## Spreadsheet policy

The viewer does not contain a formula engine. During open it reads every worksheet, retains the saved cell value, and removes the formula from the render model. This also disables the upstream renderer's convenience recalculation of volatile `TODAY()` and `NOW()` cells.

If a formula cell has no saved value, its formula is shown as text with a leading
`=` and the document receives a `fidelity-degraded` warning containing the
affected cell count. `DocumentInfo.sheets` exposes sheet names, used row/column
bounds, frozen rows/columns, merged ranges, default/custom row and column sizes,
hidden zero-sized bands, headers, and RTL layout metadata. These sizes are
unscaled CSS pixels keyed by 1-based row/column indexes.

Attached sheets use a dedicated virtual spreadsheet surface rather than page
slots: its spacer covers the full used range plus enough trailing blank bands
to fill the current viewport, and its single canvas renders only the visible
region. `RenderViewport.sheetRange` selects that 1-based window;
`scrollOffsetX/Y` describes the clipped part of its first row/column. Frozen
panes are supplied to the renderer. Public document/sheet indices remain
0-based, and `fitWidth`/`fitPage` operate on the used range rather than A4.

## Active content and network policy

- VBA projects in DOCM/XLSM/PPTM are never loaded or executed. Opening one emits `unsupported-feature`.
- Embedded package images are read from the same archive. External OOXML relationships are not fetched by the adapter.
- Text-map hyperlinks are data-only hit targets. External targets are accepted only for absolute `http:`, `https:`, `mailto:`, and `tel:` URLs. `javascript:`, `data:`, `file:`, relative URLs, and malformed targets are omitted.
- Internal Word bookmarks, slide jumps, and workbook locations remain typed internal targets. Resolvable Word/PowerPoint targets also contain a normalized 0-based `pageIndex`.
- Password-protected OOXML-in-OLE containers and backend encryption errors return `encrypted-document`; passwords are not accepted in v1.

## Fidelity and known limitations

The goal is practical viewing fidelity, not editing compatibility. Modern documents use the feature set of pinned `@silurus/ooxml@0.72.2`; unsupported equations (the optional math bundle is not included), embedded OLE objects, uncommon effects, and malformed sheet parts can degrade. A partially parsed sheet and an XLS/PPT legacy normalization both produce explicit `fidelity-degraded` warnings.

DOC never enters the heuristic `office_oxide` DOC converter. The project-owned path parses CFB/FIB/CLX, STSH, PAPX/CHPX, sections, headers/footers, table grids/merges/borders/padding, `PlfLst`/`PlfLfo` numbering, page fields, footnotes/endnotes, comment authors/bodies/point or ranged anchors and supported media records, then serializes only source-backed structure. Comment and numbering parts are added to the generated in-memory DOCX with explicit content type and relationship records; no temporary file is used. Current gaps include Word 6, exotic list/style variants, complex floating/nested images, old or compressed metafiles/PICT, advanced table shading/borders, custom note symbols and some exact pagination metrics. Unsupported active/OLE content is never executed, and the source file is never overwritten.

## Programmatic adapter configuration

```ts
import { OfficeDocumentAdapter, ViewerClient } from "@docs-viewer-wasm/viewer";

const office = new OfficeDocumentAdapter({
  legacy: {
    workerUrl: new URL("./workers/legacy-converter-worker.js", assets),
    moduleUrl: new URL("./assets/legacy/index.js", assets),
  },
});

const client = ViewerClient.create({ adapters: [office] });
```

Supplying `ViewerClientOptions.adapters` replaces the current built-in set, which is useful for custom hosting and tests. Omitting it registers the Office adapter automatically.
