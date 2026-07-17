# Legacy DOC structured conversion spike

**Decision:** `no-go` for `office_oxide@0.1.6`; recorded 2026-07-17.

**Follow-up:** `go` for an in-repository Word Binary parser; the product scope
still requires DOC. The `no-go` above applies only to shipping the stock lossy
converter, not to removing DOC from the roadmap.

## Scope and evidence

The spike inspected the pinned dependency's production DOC path and the package
binding. The available reader parses the CFB container, a limited FIB field set,
the CLX piece table, text encoding, and embedded images. Its public
`DocDocument` stores extracted text and images only. It has no parser/model for:

- STSH styles and inheritance;
- BTE PLCF plus PAPX/CHPX FKP runs and their `sprm` properties;
- section properties, page geometry, columns, headers, or footers;
- Word table row/cell boundaries, widths, borders, merges, or nested tables.

The stock DOC-to-IR converter splits extracted text into lines, promotes short
or all-uppercase lines to headings, copies the first inferred heading into the
section title, and serializes that invented structure as DOCX. This explains
duplicate titles and lost tables; successful parsing only proves text
extraction, not document fidelity.

## Decision

Extending this reader to the task's `go` threshold is a new Word Binary parser,
not a bounded adapter change. The estimated first practical subset remains
3–6 engineering weeks after a 4–6 day parser-design spike, followed by an
independently licensed structural/visual corpus and fuzzing of PLCF/FKP/sprm
bounds. No qualified permissive, browser-side drop-in converter was identified;
copyleft native/server converters remain outside the license and deployment
constraints.

The candidate audit was repeated against current primary project metadata:

- MIT `officeParser` advertises structured/browser parsing for DOCX and other
  modern/open formats, but its current input list and `fileType` enum do not
  include binary DOC: [repository](https://github.com/harshankur/officeParser).
- Apache-2.0 Apache POI has the relevant HWPF Java model, but explicitly
  requires a JVM and is not a browser/WASM package:
  [repository](https://github.com/apache/poi).
- Mature DOC conversion projects based on LibreOffice/wvWare are native and
  use licenses excluded by this project's runtime policy. They are not a
  compatible closed-commercial browser dependency.

Therefore the package:

- returns typed `fidelity-unsupported` for DOC before starting conversion;
- independently refuses DOC in the Rust bytes-out binding, preventing direct
  WASM callers from reaching the heuristic projection;
- retains `extractLegacyPlainText` only as an explicitly diagnostic API;
- continues to qualify XLS/PPT separately; their success is not evidence for
  DOC fidelity;
- keeps structured DOC as a release blocker until product scope explicitly
  removes DOC or a new permissive parser passes the gates below.

The selected follow-up is the latter: a new `legacy-doc` workspace crate will
reuse the public permissive CFB reader and generic IR/DOCX writer from
`office_oxide`, while owning FIB/CLX, formatting, section and table parsing. The
implementation stages are indexed from
[`todo/13-legacy-doc-fidelity.md`](./todo/13-legacy-doc-fidelity.md).

## Required evidence to change the decision

A future `go` needs public/generated fixtures proving FIB/CLX, STSH,
PAPX/CHPX FKP, sections, basic and nested table grids, mixed-script code pages,
images, headers/footers, corruption bounds, cancellation, WASM size/time, and
visual differential results. Plain-text equality and open-success are not
acceptable oracles.

No private diagnostic input, filename, checksum, content, screenshot, or
derived document was used in this record.
