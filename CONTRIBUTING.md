# Contributing to Zrimo

Thank you for helping improve Zrimo. Bug reports should include the browser, integration mode, document format and the smallest reproducible input that you are allowed to share. Never commit private documents or customer data.

## Development setup

Install Node.js 22.13+ or 24+, npm 11, and Rust 1.94.1 with the `wasm32-unknown-unknown` target.

```bash
npm ci
npm run build
npm run check
```

Browser work should also run:

```bash
npm run test:e2e
npm run test:e2e:matrix
```

Public qualification fixtures are downloaded into ignored `.cache/corpus/` with pinned hashes. User-provided regression files belong outside the repository or in ignored `.tmp/`; do not add them to tests, docs, screenshots or release artifacts.

## Changes

- Add focused unit and browser coverage for behavior changes.
- Keep parsing bounded and fail closed on malformed or unsupported input.
- Update public API and integration documentation with the code.
- Run `npm run audit:repository` and `npm run test:pack` before submitting packaging changes.

## License

Unless explicitly stated otherwise, contributions intentionally submitted for inclusion in Zrimo are licensed under the same `MIT OR Apache-2.0` terms as the project, without additional restrictions.
