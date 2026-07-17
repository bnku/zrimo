# Fidelity requalification — 2026-07-17

This report is a sanitized local engineering result for
`@zrimo/viewer@0.1.0-alpha.1`. It contains no private input names,
hashes, text, screenshots, or derived documents.

## Result

The qualified format subset passes its automated gates, but the package is not
a release candidate. Structured legacy DOC remains `unsupported` after the
parser spike; `release-status.json` therefore blocks alpha/beta/latest and the
pack report records `releaseCandidate: false`.

| Gate                    | Result                                                                               |
| ----------------------- | ------------------------------------------------------------------------------------ |
| TypeScript/Rust/unit    | 68 viewer tests; all workspace Rust tests and clippy pass                            |
| Native qualification    | DOC fail-closed contract plus XLS/PPT OOXML conversion/reparse pass                  |
| Chromium E2E            | 29 scenarios pass after the final rerun                                              |
| Browser matrix          | 31 pass, 1 declared Chromium-DPR2 interaction skip; Chromium, Firefox, WebKit        |
| Fidelity                | Modern DOCX, PDF, and image goldens pass; DOC has no fabricated golden               |
| Corrective format gates | DOCX selection, PDF font matrix, and 10,000×1,000 spreadsheet virtualization pass    |
| Fuzz                    | 2,000 JS mutations and four 10-second Rust targets; zero crashes/failures            |
| Dependencies            | 5 npm runtime, 60 Cargo, and 14 font assets pass license policy                      |
| Vulnerabilities         | 0 npm production and 0 Cargo advisories                                              |
| Size                    | Base 3,679,999 bytes Brotli; all optional fonts 14,640,699 bytes Brotli; pass        |
| Pack/consumers          | 307 files, recursive scan clean, private sentinel excluded, six consumer builds pass |

The packed-consumer matrix covers plain ESM/SSR, strict TypeScript, esbuild,
Vite, webpack, and Next.js/webpack. Vanilla and React production examples also
build against the package assets. No registry publish, Git tag, or external
release mutation was performed.

Machine-readable details remain in `artifacts/`, including size,
vulnerability, fuzz, browser capability, fidelity, performance, SPDX SBOM, and
pack reports. The temporary tarball stays under ignored `.cache`; the
quarantined historical artifact is not a publication source.
