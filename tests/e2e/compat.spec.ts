import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { expect, test } from "@playwright/test";

test("load, navigate, zoom, search, select, and destroy", async ({
  page,
}, testInfo) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = await import("/main.js");
    const adapter = {
      id: "compatibility",
      formats: ["pdf"],
      open: async () => ({}),
      getInfo: async () => ({ format: "pdf", unit: "page", pageCount: 2 }),
      render: async (_handle: unknown, canvas: HTMLCanvasElement) => {
        canvas.width = 320;
        canvas.height = 420;
        canvas.getContext("2d")?.fillRect(0, 0, 8, 8);
      },
      getTextMap: async (_handle: unknown, pageIndex: number) => [
        {
          text: pageIndex === 0 ? "Hello Привет" : "日本語 العربية हिन्दी",
          x: 16,
          y: 16,
          width: 240,
          height: 24,
        },
      ],
      close: () => {},
    };
    const host = document.createElement("div");
    Object.assign(host.style, { width: "640px", height: "480px" });
    document.body.append(host);
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container: host, overscan: 1 });
    await viewer.load(new TextEncoder().encode("%PDF-1.7\ncompat"), {
      fileName: "compat.pdf",
    });
    viewer.goToPage(1);
    viewer.setZoom(1.5);
    const search = await viewer.search("العربية");
    const selection = await viewer.selectText({
      startPageIndex: 0,
      startOffset: 0,
      endPageIndex: 1,
      endOffset: 3,
    });
    const capabilities = {
      userAgent: navigator.userAgent,
      offscreenCanvas: typeof OffscreenCanvas !== "undefined",
      resizeObserver: typeof ResizeObserver !== "undefined",
      createImageBitmap: typeof createImageBitmap === "function",
      bitmapRenderer:
        document.createElement("canvas").getContext("bitmaprenderer") !== null,
      fullscreen:
        typeof document.documentElement.requestFullscreen === "function",
      wasm: typeof WebAssembly === "object",
    };
    const state = { ...viewer.state };
    await viewer.destroy();
    await client.destroy();
    const removed = host.childElementCount === 0;
    host.remove();
    return {
      state,
      matches: search.matches.length,
      selected: selection.text,
      removed,
      capabilities,
    };
  });

  expect(result.state.pageIndex).toBe(1);
  expect(result.state.zoom).toBe(1.5);
  expect(result.matches).toBe(1);
  expect(result.selected).toContain("Hello Привет");
  expect(result.selected).toContain("日本語");
  expect(result.removed).toBe(true);

  const artifactDir = resolve("artifacts");
  const project = testInfo.project.name || "chromium";
  await mkdir(artifactDir, { recursive: true });
  await writeFile(
    resolve(artifactDir, `browser-capabilities-${project}.json`),
    `${JSON.stringify(
      {
        schemaVersion: 1,
        project,
        scenario: "load-navigate-zoom-search-select-destroy",
        passed: true,
        ...result.capabilities,
      },
      null,
      2,
    )}\n`,
  );
});

test("uses compatibility paths without optional browser APIs", async ({
  page,
}) => {
  await page.addInitScript(() => {
    for (const name of [
      "OffscreenCanvas",
      "ResizeObserver",
      "createImageBitmap",
    ])
      Object.defineProperty(globalThis, name, {
        configurable: true,
        value: undefined,
      });
    Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
      configurable: true,
      value: new Proxy(HTMLCanvasElement.prototype.getContext, {
        apply(target, receiver, args: [string, ...unknown[]]) {
          if (args[0] === "bitmaprenderer") return null;
          return Reflect.apply(target, receiver, args);
        },
      }),
    });
  });
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = await import("/main.js");
    const adapter = {
      id: "fallback",
      formats: ["pdf"],
      open: async () => ({}),
      getInfo: async () => ({ format: "pdf", unit: "page", pageCount: 1 }),
      render: async (_handle: unknown, canvas: HTMLCanvasElement) => {
        canvas.width = 64;
        canvas.height = 64;
        canvas.getContext("2d")!.fillRect(0, 0, 4, 4);
      },
      getTextMap: async () => [
        { text: "fallback", x: 0, y: 0, width: 50, height: 12 },
      ],
      close: () => {},
    };
    const host = document.createElement("div");
    Object.assign(host.style, { width: "320px", height: "240px" });
    document.body.append(host);
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container: host });
    await viewer.load(new TextEncoder().encode("%PDF-1.7"), {
      fileName: "fallback.pdf",
    });
    await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
    const text = await viewer.getPageText(0);
    const canvas = host.querySelector("canvas");
    await viewer.destroy();
    await client.destroy();
    host.remove();
    return { text, width: canvas?.width ?? 0 };
  });
  expect(result).toEqual({ text: "fallback", width: 64 });
});
