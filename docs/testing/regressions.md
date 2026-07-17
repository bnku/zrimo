# Regression lanes and private-input policy

The fidelity corrective work uses two deliberately separate regression lanes.
Neither lane treats a successful open or a smoke screenshot as a fidelity
oracle.

## Committed lane

`tests/regressions/manifest.json` is the release-facing qualification matrix.
Every case names an oracle, a format family, and either an executable runner or
an explicit `unsupported` state linked to its corrective task. A public fixture
must resolve through `tests/corpus/manifest.json`, which supplies an immutable
revision, source path, SPDX license, and SHA-256. A fixture-free synthetic case
uses `source.kind: "generated"`; its deterministic committed generator must be
the same executable runner named by the passing gate, so no derived binary is
stored.

`npm run test:regressions` validates this contract in CI. An `unsupported` case
keeps release promotion blocked even though ordinary development CI remains
usable. A case may be changed to `pass` only together with an executable oracle;
plain render success is not an oracle.

## Private local lane

Private diagnostic inputs are optional and never required by CI. Set
`PRIVATE_REGRESSION_DIR` to an external directory or an ignored `.tmp/`
subdirectory. That directory owns a local `manifest.json`:

```json
{
  "schemaVersion": 1,
  "cases": [
    { "id": "local-case-a", "family": "pdf", "file": "input.bin" }
  ]
}
```

Run `npm run test:regressions`. The harness validates file signatures, dispatches
only committed executable oracles, prints only `<case-id>: pass|fail`, and does
not write screenshots, extracted text, hashes, filenames, or reports. While a
format gate is `unsupported`, its matching private case intentionally reports
`fail`. Without `PRIVATE_REGRESSION_DIR`, the lane skips cleanly.

Never place the private manifest or its files under `tests/`, `packages/`,
`examples/`, `docs/`, `scripts/`, or `artifacts/`. Do not turn a private input
into a committed minimized/generated fixture. New committed fixtures must be
created independently or come from a redistributable pinned public source.

## Release reports and quarantine

`npm run test:pack` creates its candidate tarball only under ignored
`.cache/pack-test/`; it does not replace the quarantined artifact or its release
checksum. The command checks both `npm pack --dry-run` and the actual archive,
uses a temporary private-path sentinel, scans nested archive signatures, and
writes only the sanitized `artifacts/pack-report.json`.

Release inputs after corrective requalification are the exact candidate
tarball, `SHA256SUMS`, pack/size/SBOM/license/vulnerability reports, browser and
fidelity reports, and recorded real-browser results. Reports may contain case
IDs, aggregate metrics, package-relative paths, dependency coordinates, and
tool versions. They must not contain absolute paths, input filenames, hashes of
private inputs, extracted content, screenshots, or render output from the
private lane.
