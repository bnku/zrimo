# React integration examples

This application keeps one file picker above three independent integration
examples. Switching a tab destroys its `ViewerApi`/`ViewerClient`, creates the
next integration, and reopens the same browser `File` without uploading it.
Every mode uses the same animated loading overlay while lazy renderer modules,
the source bytes, legacy conversion, and document parsing are in progress.

- **Built-in UI** creates an attached viewer with `ui: true`. The package owns
  the toolbar, search, thumbnails, fullscreen button, viewport, and sheet tabs.
- **React controls** uses `ui: false`. The package owns only the virtualized
  viewport; React calls navigation, fit, zoom, search, selection-copy, and
  original-download methods and renders live API events/capabilities.
- **Headless API** omits `container`. React owns the page canvas and thumbnail
  canvases and calls `renderPage`, `renderSheetViewport`, `renderThumbnail`, and
  `getPageText` directly.

Run from the repository root:

```bash
npm run dev --workspace @zrimo/example-react
```

`vite.config.ts` serves the built package assets at `/`, matching the
`assetBaseUrl` used by the example. A production application can instead copy
the package `assets/`, `workers/`, and `fonts/` directories to its own public
asset base.
