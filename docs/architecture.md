# Architecture: modular browser document runtime

Status: updated for the fidelity-corrective baseline on 2026-07-17.

## Decision

The package is an ESM-first TypeScript facade with independently loaded format engines. A host imports only stable contracts and controller code initially; Office, legacy Office, PDF, image, CSV, SVG, font, worker, and UI assets load on demand. No document bytes leave the browser unless the host explicitly supplies a URL as the source.

```text
Host web application
        │ public events, commands, sources
        ▼
TypeScript facade / ViewerClient / DocumentViewer
        │
        ├── source + format detection + limits + cancellation
        ├── viewport, selection, search, optional basic UI
        └── worker protocol and FormatAdapter registry
                    │ lazy import by detected format
        ┌───────────┼────────────────────┬───────────────┐
        ▼           ▼                    ▼               ▼
  OOXML adapter  Legacy adapter       PDF adapter    Web adapters
  @silurus       office_oxide WASM    PDF.js worker   images/CSV/SVG
  TS + WASM          │ bytes-out      + local assets   browser APIs
        ▲            ▼
        └──── normalized OOXML bytes
```

WASM and worker assets are split by format family. OOXML parsers, legacy Office,
TIFF, and PDF.js codecs/CMaps/fonts load only for their respective formats. The
old 6 MiB PDF Rust renderer and PNG transfer bridge are no longer part of the
package; PDF.js code is lazy and its module worker/assets are self-hosted.

## Ownership boundaries

TypeScript owns host-facing API stability, source acquisition, capability detection, worker scheduling, lifecycle, viewport state, DOM/canvas composition, selection, search, localization, and UI. Imports must remain SSR-safe: no DOM access or WASM initialization occurs at module evaluation time.

Rust owns legacy conversion, shared normalized error codes, and compute-heavy
TIFF operations. PDF.js owns PDF parsing, font/CMap interpretation, canvas
rasterization, annotations, and text geometry. Rust functions return owned
buffers or serializable maps; they do not retain DOM objects or perform network
access.

`@silurus/ooxml` is treated as a qualified upstream engine rather than copied source. Its DOCX/XLSX/PPTX entry points remain lazy imports. Legacy XLS/PPT use `office_oxide::Document::to_ir`; Word 97–2003 DOC uses the project-owned bounded `legacy-doc` parser and source-backed IR projection. All three use the public `create_from_ir_to_writer` API to return OOXML bytes from memory, then enter the same modern Office path. The stock heuristic DOC projection is never called.

## Internal adapter draft

The canonical TypeScript draft is `DocumentAdapter<THandle>` in `packages/viewer/src/contracts.ts`. Each adapter declares formats and implements `open`, `metadata`, `render`, optional positioned `textRuns`, and `close`. A later runtime task will add cancellation, warnings, resource budgets, sheet/slide units, cell maps, and typed backend capabilities without exposing upstream-specific classes.

The controller owns adapter handles and guarantees one close/destroy path. Adapters may cache decoded pages or fonts, but must release WASM handles, `ImageBitmap`s, object URLs, workers, and upstream document objects when closed.

## Worker and fallback model

Parsing runs in dedicated workers in the production path. Worker messages carry request IDs, transfer `ArrayBuffer`s where ownership can move, and return structured errors. COOP/COEP, threads, SIMD, `OffscreenCanvas`, and `SharedArrayBuffer` are optional enhancements. Safari 16.4-compatible single-threaded WASM and main-thread canvas composition remain the baseline fallback.

## Security boundary

All input is untrusted. The runtime will enforce source size, decompression, entry, page, pixel, time, and cancellation limits before production adapters are marked ready. External OOXML relationships and active SVG content are blocked. Macros are never executed. Encrypted documents return a typed error rather than requesting a password in v1.

## Consequences

- A single npm package can expose one API while keeping startup and transfer costs format-specific.
- Qualified legacy conversion reuses permissive code and does not require LibreOffice, OnlyOffice, a server process, or copyleft runtime code; unsupported Word Binary features remain explicit release gaps.
- The TypeScript/Rust boundary stays coarse-grained; page buffers and text maps cross it, not individual glyph calls.
- Upstream upgrades require corpus, browser, size, license, and API qualification before changing a pin.
- Initial alpha names such as `createViewer` may change to the roadmap's `ViewerClient.create` surface during task 05; no 1.0 compatibility promise applies yet.
