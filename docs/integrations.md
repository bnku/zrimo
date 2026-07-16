# Framework and bundler integration

All recipes use the same framework-agnostic runtime. `npm run test:pack` installs the generated tarball into a clean directory and verifies strict TypeScript, plain ESM/SSR import, esbuild, Vite, webpack and Next.js production builds.

## Vite and React

Import package CSS once and create/destroy the client inside the component lifecycle. The complete runnable version is in `examples/react`.

```tsx
import { useEffect, useRef } from "react";
import { ViewerClient } from "@docs-viewer-wasm/viewer";
import "@docs-viewer-wasm/viewer/styles.css";

export function DocumentViewer({ file }: { file?: File }) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!host.current) return;
    const client = ViewerClient.create();
    const viewer = client.createViewer({ container: host.current, ui: true });
    if (file) void viewer.load(file, { fileName: file.name });
    return () => void viewer.destroy().finally(() => client.destroy());
  }, [file]);
  return <div ref={host} />;
}
```

## webpack and Next.js

The root import is SSR-safe. Create the viewer only in a client component/effect because Canvas, Worker and the host element are browser resources.

```tsx
"use client";
import { useEffect, useRef } from "react";
import { ViewerClient } from "@docs-viewer-wasm/viewer";
import "@docs-viewer-wasm/viewer/styles.css";

export default function Viewer() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const client = ViewerClient.create();
    const viewer = client.createViewer({ container: host.current!, ui: true });
    return () => void viewer.destroy().finally(() => client.destroy());
  }, []);
  return <div ref={host} />;
}
```

webpack 5 handles `new URL(..., import.meta.url)` and async WASM assets. If deployment tooling copies package assets rather than emitting them, copy `dist/assets`, `dist/workers`, and `dist/fonts` to one public directory and pass that directory as `assetBaseUrl`.

## Angular with the esbuild builder

Create the client in `ngAfterViewInit` and dispose it in `ngOnDestroy`. Angular's current application builder uses esbuild; the packed consumer test bundles the same public imports with strict browser resolution.

```ts
export class PreviewComponent implements AfterViewInit, OnDestroy {
  @ViewChild("host", { static: true }) host!: ElementRef<HTMLElement>;
  private client?: ViewerClient;
  private viewer?: ViewerApi;

  ngAfterViewInit() {
    this.client = ViewerClient.create();
    this.viewer = this.client.createViewer({
      container: this.host.nativeElement,
      ui: true,
    });
  }

  async ngOnDestroy() {
    await this.viewer?.destroy();
    await this.client?.destroy();
  }
}
```

Add the package stylesheet to `angular.json` or import it from the application stylesheet.

## Plain ESM and self-hosted assets

Serve the package from a CDN/import map, or let a bundler resolve it. For an explicit self-host directory:

```js
import { ViewerClient } from "@docs-viewer-wasm/viewer";

const client = ViewerClient.create({
  assetBaseUrl: new URL("/static/docs-viewer/", location.href),
});
```

Copy `assets/`, `workers/`, and `fonts/` from package `dist/` below that URL. Serve `.wasm` as `application/wasm`, `.js` as JavaScript modules, `.woff2` as `font/woff2`, and enable Brotli/gzip with immutable hashed deployment URLs. Workers are ESM and CSP-compatible without `eval`; allow the self-host origin in `worker-src`, `script-src`, `font-src` and `connect-src` as applicable.

