# Office formats

The built-in `OfficeDocumentAdapter` is registered by `ViewerClient` and routes modern OOXML and legacy binary Office files through one document model. Detection is content-first; file names and MIME types preserve macro/slideshow subtypes only when they agree with the detected family.

## Format matrix

| Input | Internal path | Unit | Rendering and text |
|---|---|---|---|
| DOCX, DOCM | `@silurus/ooxml` DOCX engine | page | Section-aware canvas pages and positioned text runs |
| XLSX, XLSM | `@silurus/ooxml` XLSX engine | sheet | Sheet viewport, merged ranges, frozen panes, cell coordinates and positioned text |
| PPTX, PPTM, PPSX | `@silurus/ooxml` PPTX engine | slide | Master/layout-aware canvas slides and positioned text runs |
| DOC | `office_oxide` WASM → DOCX engine | page | Same navigation/text semantics as DOCX after in-memory normalization |
| XLS | `office_oxide` WASM → XLSX engine | sheet | Same navigation/text semantics as XLSX after in-memory normalization |
| PPT | `office_oxide` WASM → PPTX engine | slide | Same navigation/text semantics as PPTX after in-memory normalization |

Modern OOXML parsing occurs in the backend's module worker. Legacy conversion occurs in the package's `legacy-converter-worker.js`: input and generated OOXML use transferable buffers, never a Blob URL, server request, native executable, or temporary file. Generated OOXML is an internal derived artifact; only the original input remains available through `getOriginalBytes()` and all parser/converter state is released on `close()`/`destroy()`.

The package distribution contains the converter worker and `assets/legacy/index.js`/`index_bg.wasm`. With a self-hosted asset directory, set `assetBaseUrl`; the expected paths are `workers/legacy-converter-worker.js` and `wasm/legacy/index.js`. `legacy.workerUrl` and `legacy.moduleUrl` can be overridden when constructing `OfficeDocumentAdapter`.

## Spreadsheet policy

The viewer does not contain a formula engine. During open it reads every worksheet, retains the saved cell value, and removes the formula from the render model. This also disables the upstream renderer's convenience recalculation of volatile `TODAY()` and `NOW()` cells.

If a formula cell has no saved value, its formula is shown as text with a leading `=` and the document receives a `fidelity-degraded` warning containing the affected cell count. `DocumentInfo.sheets` exposes sheet names, used row/column bounds, frozen rows/columns, and merged ranges. `RenderViewport.sheetRange` selects the 1-based sheet window passed to the backend; public document/sheet indices remain 0-based.

## Active content and network policy

- VBA projects in DOCM/XLSM/PPTM are never loaded or executed. Opening one emits `unsupported-feature`.
- Embedded package images are read from the same archive. External OOXML relationships are not fetched by the adapter.
- Text-map hyperlinks are data-only hit targets. External targets are accepted only for absolute `http:`, `https:`, `mailto:`, and `tel:` URLs. `javascript:`, `data:`, `file:`, relative URLs, and malformed targets are omitted.
- Internal Word bookmarks, slide jumps, and workbook locations remain typed internal targets. Resolvable Word/PowerPoint targets also contain a normalized 0-based `pageIndex`.
- Password-protected OOXML-in-OLE containers and backend encryption errors return `encrypted-document`; passwords are not accepted in v1.

## Fidelity and known limitations

The goal is practical viewing fidelity, not editing compatibility. Modern documents use the feature set of pinned `@silurus/ooxml@0.72.2`; unsupported equations (the optional math bundle is not included), embedded OLE objects, uncommon effects, and malformed sheet parts can degrade. A partially parsed sheet and a legacy normalization both produce explicit `fidelity-degraded` warnings.

Legacy conversion is necessarily lossy for constructs that `office_oxide@0.1.6` cannot represent in its intermediate model. The source file is never overwritten. Golden thresholds and broader adversarial/fidelity corpora are release gates in roadmap task 07 rather than a claim of pixel identity for every Office producer.

## Programmatic adapter configuration

```ts
import { OfficeDocumentAdapter, ViewerClient } from "@docs-viewer-wasm/viewer";

const office = new OfficeDocumentAdapter({
  legacy: {
    workerUrl: new URL("./workers/legacy-converter-worker.js", assets),
    moduleUrl: new URL("./wasm/legacy/index.js", assets),
  },
});

const client = ViewerClient.create({ adapters: [office] });
```

Supplying `ViewerClientOptions.adapters` replaces the current built-in set, which is useful for custom hosting and tests. Omitting it registers the Office adapter automatically.
