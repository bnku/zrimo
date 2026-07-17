# Release checklist

## Corrective quarantine

The existing `0.1.0-alpha.0` artifact failed fidelity qualification and must not
be promoted, republished, or used as the base for `beta`/`latest`. Before any
publication attempt, run `npm run release:gate -- --channel <channel>`; while
`release-status.json` is `blocked`, failure is the required result. Only task 14
may switch the status after every corrective format gate is executable and
green. `npm run test:pack` creates a temporary non-candidate under `.cache` and
does not update release artifacts.

## Candidate preparation

- Freeze the public API and update version/changelog without rewriting an existing tag.
- Run `npm ci`, `npm run check`, `npm run test:qualification`, `npm run test:e2e`, `npm run test:e2e:matrix`, `npm run fuzz:js`, and the scheduled Rust fuzz budget.
- Confirm `tests/regressions/manifest.json` has no `unsupported` cases and run the committed regression lane without private inputs.
- Run `npm run audit:vulnerabilities`, `npm run report:size`, `npm run report:sbom`, and `npm run test:pack`.
- Confirm SSIM, performance, size, license and vulnerability JSON artifacts are present and green.
- Test the packed tarball on real current/previous Chrome, Edge, Firefox and Safari; record browser/OS versions.
- Verify offline font mode, original download, self-host/base URL, MIME types, CSP and that no unexpected network/telemetry request occurs.

## Artifact review

- Compare `artifacts/SHA256SUMS`, `pack-report.json`, `size-report.json`, `sbom.spdx.json` and third-party notices.
- Confirm the tarball contains no source corpus, maps, credentials, environment files or development-only sources.
- Confirm recursive content scanning and the temporary private sentinel gates are green; private-lane inputs and outputs must not be present in reports.
- Install the exact tarball into clean Vite, webpack/Next, esbuild/Angular and plain ESM consumers.
- Store CI logs, browser capability reports and provenance with the release.

## Channels and publication

1. Publish an immutable `alpha` tag for API/integration feedback.
2. After blockers are resolved, publish `beta` with frozen public contracts.
3. Promote to `latest`/1.0 only after all checklist gates and real-browser smoke tests pass.

Publication requires an authorized maintainer and registry credentials; automation must use npm trusted publishing/provenance where available. This repository preparation does not publish or create an external tag by itself.

## Semver and deprecation

After 1.0, breaking public API changes require a major version. New backward-compatible API uses minor versions; fixes and fidelity/security corrections use patch versions. Deprecations remain documented for at least one minor release before removal unless retaining the path creates an active security vulnerability.
