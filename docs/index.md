---
layout: home

hero:
  name: Zrimo
  text: Any document. One canvas.
  tagline: Render Office, PDF, images and structured data inside your web app. Files stay in the browser; Rust/WASM modules load only when a format needs them.
  image:
    src: /logo.svg
    alt: Zrimo
  actions:
    - theme: brand
      text: Open React demo
      link: /demo/
      target: _self
    - theme: alt
      text: Get started
      link: /getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/bnku/zrimo

features:
  - icon:
      src: /icons/document-duplicate.svg
      alt: ""
      width: 24
      height: 24
    title: Broad format coverage
    details: DOCX, XLSX, PPTX, qualified Word DOC, legacy XLS/PPT, PDF, raster and TIFF images, SVG, CSV and TSV.
    link: /compatibility
    linkText: Format support
  - icon:
      src: /icons/shield-check.svg
      alt: ""
      width: 24
      height: 24
    title: Browser-private
    details: Documents are parsed and rendered locally. There is no conversion service, telemetry, or implicit fetch of document relationships.
    link: /security
    linkText: Security model
  - icon:
      src: /icons/window.svg
      alt: ""
      width: 24
      height: 24
    title: Embeddable by design
    details: Use the built-in UI, hide it and drive ViewerApi from React, or work headlessly with page rendering and text maps.
    link: /integrations
    linkText: Integration modes
  - icon:
      src: /icons/language.svg
      alt: ""
      width: 24
      height: 24
    title: Multilingual
    details: Lazy self-hosted fallback fonts cover Latin, Cyrillic, CJK, Arabic scripts and the supported Indic writing systems.
    link: /fonts
    linkText: Font coverage
  - icon:
      src: /icons/cursor-arrow-rays.svg
      alt: ""
      width: 24
      height: 24
    title: Real interactions
    details: Virtualized pages and sheets, pan, zoom, fit modes, search, selectable text, multi-cell selection and TSV clipboard output.
    link: /ui
    linkText: Viewer controls
  - icon:
      src: /icons/rectangle-stack.svg
      alt: ""
      width: 24
      height: 24
    title: Virtualized and bounded
    details: Pages and sheets stay virtualized around the viewport, while configurable limits bound input, decoded pixels, text maps and worker time.
    link: /performance
    linkText: Performance model
---

<div class="zrimo-proof">
  <div><strong>0 uploads</strong><span>Local files stay inside the browser runtime.</span></div>
  <div><strong>3 modes</strong><span>Built-in UI, custom framework controls, or headless.</span></div>
  <div><strong>2 licenses</strong><span>Choose MIT or Apache-2.0 for your integration.</span></div>
</div>

## A small API with room to customize

```ts
import { ViewerClient } from "@zrimo/viewer";
import "@zrimo/viewer/styles.css";

const client = ViewerClient.create({
  assetBaseUrl: new URL("/zrimo-assets/", location.href),
});
const viewer = client.createViewer({
  container: document.querySelector("#viewer")!,
  ui: true,
  fit: "width",
});

await viewer.load(file, { fileName: file.name });
```

The <a href="./demo/" target="_self">React demo</a> runs the real package and shows all three integration modes. Nothing is uploaded when you open a document.
