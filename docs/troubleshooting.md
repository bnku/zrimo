# Troubleshooting

## Worker or WASM asset returns 404

Prefer normal package imports so the bundler can emit `new URL(..., import.meta.url)` assets. With manual/CDN copying, preserve `assets/`, `workers/` and `fonts/` and pass their common public root as `assetBaseUrl`.

## WASM has the wrong MIME type

Configure the static server to return `application/wasm`. The generated bindings can fall back from streaming instantiation, but the correct MIME type is faster and avoids server/browser differences.

## A document returns `resource-limit`

Inspect the typed error `details`. Limits cover input/expanded archive bytes, ZIP entries, pixels, SVG bytes, CSV cells, text maps, document units, concurrent renders and operation time. Raise only the specific limit after validating the input source and measuring memory on target devices.

## Fonts are missing or a network request is unexpected

Use `fontPolicy: { mode: "offline" }` for zero font fetches, register application fonts explicitly, or self-host `dist/fonts`. Auto mode fetches only package-owned Noto packs for scripts encountered in the document; it never contacts Google Fonts.

## SSR says DOM/Worker is unavailable

Import is SSR-safe, but viewer creation with a container and document loading belong in the browser lifecycle (`useEffect`, `ngAfterViewInit`, or equivalent). Headless construction is safe; canvas rendering still requires browser Canvas APIs.

## Office fidelity differs from the authoring application

The viewer is display-only and prioritizes safe practical fidelity. Legacy Office is converted to OOXML, formulas use stored cached values, macros never execute, image documents have no OCR, and unsupported features emit warnings. Check `viewer.getDocumentInfo().warnings` and the compatibility matrix.

