# Security model

## Trust boundary

Document bytes, filenames, MIME headers, URLs, archive metadata, embedded fonts/images, hyperlinks, and rendered text are untrusted. The browser host, package assets selected through `assetBaseUrl`, and code pins in the lockfiles are trusted. The runtime has no server fallback and no telemetry.

## Default limits

| Limit                      |     Default | Enforcement point                                                 |
| -------------------------- | ----------: | ----------------------------------------------------------------- |
| Input bytes                |     100 MiB | Blob/Content-Length before read; streamed byte count during fetch |
| Expanded Office package    |     512 MiB | ZIP central directory before parser open                          |
| Single ZIP entry           |      64 MiB | ZIP central directory before parser open                          |
| Decoded raster pixels      | 100 million | PNG/GIF/JPEG/WebP/BMP/TIFF headers before browser/Rust decode     |
| SVG source                 |      16 MiB | Before DOM parsing/sanitization                                   |
| CSV/TSV cells              |   1 million | During bounded parsing                                            |
| Cached text maps           |      64 MiB | Before retaining positioned text                                  |
| Pages/slides/sheets/images |     100,000 | Before publishing ready state                                     |
| Concurrent renders         |           2 | Shared priority scheduler                                         |
| Worker operation           |  30 seconds | Timer terminates the worker and returns `resource-limit`          |

Limits are positive safe integers and can be tightened or explicitly raised per client or load. ZIP64 entries are rejected until the production archive inspector supports their 64-bit metadata without ambiguity. The central-directory checks are a first barrier; adapters must also enforce limits while inflating because archive metadata can lie.

## Isolation and cancellation

Production parsing/conversion executes through dedicated ESM workers where the backend permits it. RPC requests have monotonically increasing IDs. `AbortSignal` produces a `cancel` message; worker handlers receive their own signal and return a typed `aborted` failure. A crash rejects every pending request with `worker-crashed`. An operation timeout returns `resource-limit` and terminates the worker; close/destroy also terminates workers so WASM linear memory is released.

The baseline does not require `SharedArrayBuffer`, threads, COOP/COEP, SIMD, or `OffscreenCanvas`. This avoids making cross-origin isolation a security prerequisite.

## Active content

Macros are display-only data and are never executed. Runtime code does not evaluate document JavaScript, embedded executables, OLE actions, launch actions, or scripts. Office adapters do not fetch external relationships. The SVG adapter removes scripts, styles, event attributes, foreign/embedded active content, and non-fragment resource references before decoding. External hyperlinks remain data until the host-approved navigation hook handles a sanitized scheme.

Encrypted Office/PDF is outside v1 and returns `encrypted-document`. The viewer must not silently try empty/default passwords or upload the document elsewhere.

## Network and privacy

Only an explicit URL source and package-owned assets may cause network traffic. Source requests use the host's custom `fetch`, allowing authentication and policy enforcement. External document relationships are not fetched. Font network policy is explicit (`auto`, offline, or custom resolver). Packaged fallback fonts are local assets with pinned SHA-256 hashes.

Original bytes are held only until close/destroy. Logging hooks receive stable codes and safe details, not document content or raw bytes. Hosts remain responsible for URL authorization, CSP, allowed download/navigation behaviour, and choosing stricter limits for their environment.

## Verification

The parsers are exercised with malformed, truncated, encrypted, oversized and
adversarial inputs, mutation fuzzing and worker lifecycle tests. Dependency and
license audits complement those tests. This is engineering evidence, not a
security certification; hosts must still apply their own threat model, CSP,
URL authorization and resource limits.

Please report suspected vulnerabilities through the repository's
[security policy](https://github.com/bnku/zimo/security/policy) instead of a
public issue.
