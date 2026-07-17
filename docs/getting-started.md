# Getting started

Zrimo is a browser-only viewer. Your application owns the file source and the container; Zrimo owns format detection, lazy adapter loading, rendering and interaction state.

## Install

```bash
npm install @zrimo/viewer
```

The package ships ESM, TypeScript declarations, workers, Rust/WASM modules, PDF.js resources, CSS and optional self-hosted fonts. Its build tooling requires Node.js 22.13+ or 24+ because the PDF.js dependency uses that engine baseline.

## Create a viewer

```ts
import { ViewerClient } from "@zrimo/viewer";
import "@zrimo/viewer/styles.css";

const client = ViewerClient.create({
  assetBaseUrl: new URL("/vendor/zrimo/", location.href),
});

const viewer = client.createViewer({
  container: document.querySelector("#viewer")!,
  ui: true,
  fit: "width",
});

await viewer.load(file, { fileName: file.name });
```

Give the host an explicit height:

```css
#viewer {
  width: 100%;
  height: 100dvh;
  min-height: 320px;
}
```

## Serve runtime assets

Copy the runtime directories into your application's public output. The package includes a small build-time CLI for this:

```bash
npx zrimo-copy-assets public/vendor/zrimo
```

It copies these directories from `node_modules/@zrimo/viewer/dist/` to the destination:

```text
assets/
fonts/
workers/
```

The URL must end with `/`. Keep the directory structure unchanged and serve `.wasm` as `application/wasm` and `.mjs` as JavaScript. The package does not load fonts or format modules until they are needed.

Run the command after dependency installation and before your application build. The bundled [React demo](./demo/) uses the package distribution itself as its public asset root.

## Clean up

Destroy both levels when the host view unmounts:

```ts
await viewer.destroy();
await client.destroy();
```

`destroy()` cancels active work, releases workers and removes listeners. See [framework integrations](./integrations.md) for lifecycle examples and the [API reference](./api/reference.md) for every option and event.
