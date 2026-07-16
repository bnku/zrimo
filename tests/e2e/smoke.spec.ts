import { expect, test } from "@playwright/test";

test("loads the ESM package in Chromium", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Viewer status: idle")).toBeVisible();
});

test("virtualizes a long document and exposes viewport interactions", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as {
      ViewerClient: {
        create(options: { adapters: readonly unknown[] }): {
          createViewer(options: { container: HTMLElement; overscan: number }): {
            load(
              source: Uint8Array,
              options: { fileName: string },
            ): Promise<void>;
            readonly state: {
              pageIndex: number;
              zoom: number;
              panY: number;
            };
            goToPage(pageIndex: number): void;
            fitWidth(): void;
            panBy(deltaX: number, deltaY: number): void;
            destroy(): Promise<void>;
          };
          destroy(): Promise<void>;
        };
      };
    };
    const adapter = {
      id: "e2e-virtualized",
      formats: ["pdf"],
      open: async () => ({}),
      getInfo: async () => ({ format: "pdf", unit: "page", pageCount: 10_000 }),
      render: async (
        _handle: unknown,
        target: HTMLCanvasElement,
        viewport: { zoom: number },
      ) => {
        target.width = Math.round(816 * viewport.zoom);
        target.height = Math.round(1056 * viewport.zoom);
      },
      getTextMap: async (_handle: unknown, pageIndex: number) => [
        {
          text: `Страница ${pageIndex + 1} — 日本語 العربية हिन्दी`,
          x: 24,
          y: 24,
          width: 420,
          height: 24,
        },
      ],
      close: () => {},
    };
    const container = document.createElement("div");
    Object.assign(container.style, { width: "800px", height: "600px" });
    document.body.append(container);
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container, overscan: 1 });
    await viewer.load(new TextEncoder().encode("%PDF-1.7\n"), {
      fileName: "long.pdf",
    });
    const nextFrame = () =>
      new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    await nextFrame();
    await nextFrame();
    const root = container.querySelector<HTMLElement>(
      '[data-docs-viewer="viewport"]',
    )!;
    const initialSlotCount = root.querySelectorAll("canvas").length;
    const initialText = root.querySelector("[data-start]")?.textContent ?? "";

    viewer.goToPage(5_000);
    await nextFrame();
    await nextFrame();
    const deepIndices = Array.from(
      root.querySelectorAll<HTMLCanvasElement>("canvas"),
      (canvas) => Number(canvas.parentElement?.dataset.pageIndex),
    );
    viewer.fitWidth();
    const fittedZoom = viewer.state.zoom;
    root.dispatchEvent(
      new WheelEvent("wheel", {
        deltaY: -100,
        ctrlKey: true,
        cancelable: true,
      }),
    );
    const wheelZoom = viewer.state.zoom;
    root.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    const keyboardPanY = viewer.state.panY;
    const touch = (type: string, pointerId: number, clientX: number) =>
      root.dispatchEvent(
        new PointerEvent(type, {
          pointerId,
          pointerType: "touch",
          clientX,
          clientY: 120,
          button: 0,
          buttons: type === "pointerup" ? 0 : 1,
          bubbles: true,
          cancelable: true,
        }),
      );
    touch("pointerdown", 11, 100);
    touch("pointerdown", 12, 200);
    touch("pointermove", 12, 260);
    const pinchZoom = viewer.state.zoom;
    touch("pointerup", 12, 260);
    touch("pointerup", 11, 100);
    viewer.panBy(0, 2_000);
    await nextFrame();
    await nextFrame();
    const finalSlotCount = root.querySelectorAll("canvas").length;
    const finalPage = viewer.state.pageIndex;
    const panY = viewer.state.panY;
    await viewer.destroy();
    const viewportRemoved = !container.querySelector(
      '[data-docs-viewer="viewport"]',
    );
    await client.destroy();
    container.remove();
    return {
      initialSlotCount,
      initialText,
      deepIndices,
      fittedZoom,
      wheelZoom,
      keyboardPanY,
      pinchZoom,
      finalSlotCount,
      finalPage,
      panY,
      viewportRemoved,
    };
  });

  expect(result.initialSlotCount).toBeLessThanOrEqual(4);
  expect(result.initialText).toContain("日本語 العربية हिन्दी");
  expect(result.deepIndices.some((pageIndex) => pageIndex >= 4_999)).toBe(true);
  expect(result.fittedZoom).toBeGreaterThan(0.9);
  expect(result.wheelZoom).toBeGreaterThan(result.fittedZoom);
  expect(result.keyboardPanY).toBeGreaterThan(0);
  expect(result.pinchZoom).toBeGreaterThan(result.wheelZoom);
  expect(result.finalSlotCount).toBeLessThanOrEqual(4);
  expect(result.finalPage).toBeGreaterThan(5_000);
  expect(result.panY).toBeGreaterThan(0);
  expect(result.viewportRemoved).toBe(true);
});

test("drags and keyboard-extends spreadsheet cell selections", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as {
      ViewerClient: {
        create(options: { adapters: readonly unknown[] }): {
          createViewer(options: { container: HTMLElement }): {
            load(
              source: Uint8Array,
              options: { fileName: string },
            ): Promise<void>;
            getSelection(): {
              startRow: number;
              startColumn: number;
              endRow: number;
              endColumn: number;
            } | null;
            copySelection(): Promise<string>;
            destroy(): Promise<void>;
          };
          destroy(): Promise<void>;
        };
      };
    };
    const cells = [
      { text: "A", row: 1, column: 1, x: 20, y: 20 },
      { text: "B", row: 1, column: 2, x: 140, y: 20 },
      { text: "C", row: 2, column: 1, x: 20, y: 70 },
      { text: "D", row: 2, column: 2, x: 140, y: 70 },
    ].map((cell) => ({ ...cell, width: 100, height: 40 }));
    const adapter = {
      id: "e2e-sheet-selection",
      formats: ["csv"],
      open: async () => ({}),
      getInfo: async () => ({
        format: "csv",
        unit: "sheet",
        pageCount: 1,
        sheetNames: ["Sheet 1"],
        sheets: [
          {
            name: "Sheet 1",
            frozenRows: 0,
            frozenColumns: 0,
            mergedRanges: [],
            maxRow: 2,
            maxColumn: 2,
          },
        ],
      }),
      render: async (_handle: unknown, target: HTMLCanvasElement) => {
        target.width = 816;
        target.height = 1056;
      },
      getTextMap: async () => cells,
      close: () => {},
    };
    const container = document.createElement("div");
    Object.assign(container.style, { width: "800px", height: "600px" });
    document.body.append(container);
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container });
    await viewer.load(new TextEncoder().encode("A,B\nC,D"), {
      fileName: "cells.csv",
    });
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
    const root = container.querySelector<HTMLElement>(
      '[data-docs-viewer="viewport"]',
    )!;
    const first = root.querySelector<HTMLElement>(
      '[data-row="1"][data-column="1"]',
    )!;
    const last = root.querySelector<HTMLElement>(
      '[data-row="2"][data-column="2"]',
    )!;
    const start = first.getBoundingClientRect();
    const end = last.getBoundingClientRect();
    const pointer = (
      target: HTMLElement,
      type: string,
      x: number,
      y: number,
      buttons: number,
    ) =>
      target.dispatchEvent(
        new PointerEvent(type, {
          pointerId: 21,
          pointerType: "mouse",
          clientX: x,
          clientY: y,
          button: 0,
          buttons,
          bubbles: true,
        }),
      );
    pointer(first, "pointerdown", start.x + 4, start.y + 4, 1);
    pointer(root, "pointermove", end.x + 4, end.y + 4, 1);
    pointer(root, "pointerup", end.x + 4, end.y + 4, 0);
    const dragged = viewer.getSelection();

    pointer(first, "pointerdown", start.x + 4, start.y + 4, 1);
    pointer(root, "pointerup", start.x + 4, start.y + 4, 0);
    root.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", shiftKey: true }),
    );
    root.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", shiftKey: true }),
    );
    const keyboard = viewer.getSelection();
    const copied = await viewer.copySelection();
    await viewer.destroy();
    await client.destroy();
    container.remove();
    return { dragged, keyboard, copied };
  });

  expect(result.dragged).toMatchObject({
    startRow: 1,
    startColumn: 1,
    endRow: 2,
    endColumn: 2,
  });
  expect(result.keyboard).toMatchObject({
    startRow: 1,
    startColumn: 1,
    endRow: 2,
    endColumn: 2,
  });
  expect(result.copied).toBe("A\tB\nC\tD");
});

test("runs the localized basic UI workflow without leaking styles", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as {
      ViewerClient: {
        create(options: { adapters: readonly unknown[]; assetBaseUrl: URL }): {
          createViewer(options: {
            container: HTMLElement;
            ui: boolean;
            locale: "ru";
            translations: { download: string };
          }): {
            load(
              source: Uint8Array,
              options: { fileName: string },
            ): Promise<void>;
            readonly state: { pageIndex: number; zoom: number };
            destroy(): Promise<void>;
          };
          destroy(): Promise<void>;
        };
      };
    };
    const adapter = {
      id: "e2e-basic-ui",
      formats: ["pdf"],
      open: async () => ({}),
      getInfo: async () => ({ format: "pdf", unit: "page", pageCount: 3 }),
      render: async (
        _handle: unknown,
        target: HTMLCanvasElement,
        viewport: { zoom: number },
      ) => {
        target.width = Math.round(816 * viewport.zoom);
        target.height = Math.round(1056 * viewport.zoom);
      },
      getTextMap: async (_handle: unknown, pageIndex: number) => [
        {
          text: `Привет на странице ${pageIndex + 1}`,
          x: 24,
          y: 24,
          width: 240,
          height: 24,
        },
      ],
      close: () => {},
    };
    const hostProbe = document.createElement("button");
    hostProbe.id = "host-style-probe";
    hostProbe.textContent = "Host";
    document.body.append(hostProbe);
    const container = document.createElement("div");
    container.id = "basic-ui-host";
    Object.assign(container.style, { width: "900px", height: "680px" });
    document.body.append(container);
    const client = ViewerClient.create({
      adapters: [adapter],
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer({
      container,
      ui: true,
      locale: "ru",
      translations: { download: "Сохранить исходник" },
    });
    await viewer.load(new TextEncoder().encode("%PDF-1.7\nbasic-ui"), {
      fileName: "ui-source.pdf",
    });
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
    (
      window as Window & {
        __stage6?: { viewer: typeof viewer; client: typeof client };
      }
    ).__stage6 = { viewer, client };
  });

  const ui = page.locator('[data-docs-viewer-ui="root"]');
  await expect(ui).toBeVisible();
  await expect(
    ui.getByRole("button", { name: "Сохранить исходник" }),
  ).toBeVisible();
  await expect(ui.getByRole("button", { name: "Поиск" })).toBeVisible();
  expect(
    await page
      .locator("#host-style-probe")
      .evaluate((element) => getComputedStyle(element).borderTopStyle),
  ).not.toBe("solid");

  await ui.getByRole("button", { name: "Вперёд" }).click();
  expect(
    await page.evaluate(
      () =>
        (
          window as Window & {
            __stage6?: { viewer: { state: { pageIndex: number } } };
          }
        ).__stage6?.viewer.state.pageIndex,
    ),
  ).toBe(1);
  await ui.getByRole("button", { name: "Увеличить" }).click();
  expect(
    await page.evaluate(
      () =>
        (
          window as Window & {
            __stage6?: { viewer: { state: { zoom: number } } };
          }
        ).__stage6?.viewer.state.zoom,
    ),
  ).toBeGreaterThan(1);

  await ui.getByRole("button", { name: "Поиск" }).click();
  const search = ui.getByRole("searchbox", { name: "Поиск" });
  await search.fill("привет");
  await expect(ui.locator(".docs-viewer-ui__search-status")).toHaveText("1/3");
  await ui.getByRole("button", { name: "Миниатюры" }).click();
  await expect(ui.locator('[data-panel="thumbnails"] canvas')).toHaveCount(3);

  await ui.getByRole("button", { name: "На весь экран" }).click();
  const fullscreenActive = await ui.evaluate(
    (element) =>
      element.classList.contains("docs-viewer-ui--fullscreen-fallback") ||
      document.fullscreenElement === element,
  );
  expect(fullscreenActive).toBe(true);
  await ui.press("Escape");

  const download = page.waitForEvent("download");
  await ui.getByRole("button", { name: "Сохранить исходник" }).click();
  expect((await download).suggestedFilename()).toBe("ui-source.pdf");

  await page.evaluate(async () => {
    const stage = (
      window as Window & {
        __stage6?: {
          viewer: { destroy(): Promise<void> };
          client: { destroy(): Promise<void> };
        };
      }
    ).__stage6;
    await stage?.viewer.destroy();
    await stage?.client.destroy();
    delete (window as Window & { __stage6?: unknown }).__stage6;
    document.querySelector("#basic-ui-host")?.remove();
    document.querySelector("#host-style-probe")?.remove();
  });
  await expect(ui).toHaveCount(0);
});

test("loads only encountered local font packs", async ({ page }) => {
  const fontRequests: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname.startsWith("/fonts/"))
      fontRequests.push(new URL(request.url()).pathname);
  });
  await page.goto("/");
  const warnings = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as {
      ViewerClient: {
        create(options: { adapters: readonly unknown[]; assetBaseUrl: URL }): {
          createViewer(): {
            load(
              source: Uint8Array,
              options: { fileName: string },
            ): Promise<void>;
            renderPage(
              pageIndex: number,
              canvas: HTMLCanvasElement,
            ): Promise<void>;
            on(
              type: "warning",
              listener: (warning: { code: string }) => void,
            ): () => void;
            destroy(): Promise<void>;
          };
          destroy(): Promise<void>;
        };
      };
    };
    const adapter = {
      id: "e2e-font-policy",
      formats: ["pdf"],
      open: async () => ({}),
      getInfo: async () => ({ format: "pdf", unit: "page", pageCount: 1 }),
      render: async (_handle: unknown, target: HTMLCanvasElement) => {
        target.width = 200;
        target.height = 100;
      },
      getTextMap: async () => [
        { text: "العربية हिन्दी", x: 0, y: 0, width: 180, height: 24 },
      ],
      close: () => {},
    };
    const client = ViewerClient.create({
      adapters: [adapter],
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer();
    const warnings: string[] = [];
    viewer.on("warning", (warning) => warnings.push(warning.code));
    await viewer.load(new TextEncoder().encode("%PDF-1.7\nfonts"), {
      fileName: "fonts.pdf",
    });
    await viewer.renderPage(0, document.createElement("canvas"));
    await viewer.renderPage(0, document.createElement("canvas"));
    await viewer.destroy();
    await client.destroy();
    return warnings;
  });

  expect(fontRequests.sort()).toEqual([
    "/fonts/noto-sans-arabic.woff2",
    "/fonts/noto-sans-devanagari.woff2",
  ]);
  expect(warnings).toEqual([]);
});

test("shows capability-driven sheet tabs and selected range", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as {
      ViewerClient: {
        create(options: { adapters: readonly unknown[] }): {
          createViewer(options: { container: HTMLElement; ui: boolean }): {
            load(
              source: Uint8Array,
              options: { fileName: string },
            ): Promise<void>;
            selectCells(range: {
              sheetIndex: number;
              startRow: number;
              startColumn: number;
              endRow: number;
              endColumn: number;
            }): unknown;
            readonly state: { pageIndex: number };
            destroy(): Promise<void>;
          };
          destroy(): Promise<void>;
        };
      };
    };
    const sheet = (name: string) => ({
      name,
      frozenRows: 0,
      frozenColumns: 0,
      mergedRanges: [],
      maxRow: 20,
      maxColumn: 10,
    });
    const adapter = {
      id: "e2e-ui-sheets",
      formats: ["csv"],
      open: async () => ({}),
      getInfo: async () => ({
        format: "csv",
        unit: "sheet",
        pageCount: 2,
        sheetNames: ["Data", "Summary"],
        sheets: [sheet("Data"), sheet("Summary")],
      }),
      render: async (_handle: unknown, target: HTMLCanvasElement) => {
        target.width = 816;
        target.height = 600;
      },
      getTextMap: async () => [
        {
          text: "value",
          row: 1,
          column: 1,
          x: 10,
          y: 10,
          width: 80,
          height: 20,
        },
      ],
      close: () => {},
    };
    const container = document.createElement("div");
    Object.assign(container.style, { width: "800px", height: "600px" });
    document.body.append(container);
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container, ui: true });
    await viewer.load(new TextEncoder().encode("value\n1"), {
      fileName: "sheets.csv",
    });
    const root = container.querySelector<HTMLElement>(
      '[data-docs-viewer-ui="root"]',
    )!;
    const summary = root.querySelector<HTMLButtonElement>(
      '[data-action="sheet-1"]',
    )!;
    summary.click();
    viewer.selectCells({
      sheetIndex: 1,
      startRow: 2,
      startColumn: 3,
      endRow: 4,
      endColumn: 5,
    });
    const output = {
      pageIndex: viewer.state.pageIndex,
      tabs: Array.from(
        root.querySelectorAll<HTMLButtonElement>('[data-action^="sheet-"]'),
        (button) => button.textContent,
      ),
      range: root.querySelector(".docs-viewer-ui__sheets")?.textContent ?? "",
      thumbnailsHidden:
        root.querySelector<HTMLButtonElement>('[data-action="thumbnails"]')
          ?.hidden ?? false,
      pageControlsHidden:
        root.querySelector<HTMLElement>('[data-control="page"]')?.hidden ??
        false,
    };
    await viewer.destroy();
    await client.destroy();
    container.remove();
    return output;
  });

  expect(result.pageIndex).toBe(1);
  expect(result.tabs).toEqual(["Data", "Summary"]);
  expect(result.range).toContain("R2C3:R4C5");
  expect(result.thumbnailsHidden).toBe(true);
  expect(result.pageControlsHidden).toBe(true);
});

test("loads the complete multilingual fallback corpus without missing packs", async ({
  page,
}) => {
  const fontRequests = new Set<string>();
  page.on("request", (request) => {
    const pathname = new URL(request.url()).pathname;
    if (pathname.startsWith("/fonts/") && pathname.endsWith(".woff2"))
      fontRequests.add(pathname);
  });
  await page.goto("/");
  const warnings = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as {
      ViewerClient: {
        create(options: { adapters: readonly unknown[]; assetBaseUrl: URL }): {
          createViewer(): {
            load(
              source: Uint8Array,
              options: { fileName: string },
            ): Promise<void>;
            renderPage(
              pageIndex: number,
              canvas: HTMLCanvasElement,
            ): Promise<void>;
            on(
              type: "warning",
              listener: (warning: { code: string }) => void,
            ): () => void;
            destroy(): Promise<void>;
          };
          destroy(): Promise<void>;
        };
      };
    };
    const text = [
      "The quick brown fox Съешь ещё",
      "简体中文 繁體中文 日本語 カタカナ 한국어",
      "العَرَبِيَّة فارسی اردو",
      "हिन्दी বাংলা ગુજરાતી ਪੰਜਾਬੀ ଓଡ଼ିଆ தமிழ் తెలుగు ಕನ್ನಡ മലയാളം",
    ].join("\n");
    const adapter = {
      id: "e2e-language-corpus",
      formats: ["pdf"],
      open: async () => ({}),
      getInfo: async () => ({ format: "pdf", unit: "page", pageCount: 1 }),
      render: async (_handle: unknown, target: HTMLCanvasElement) => {
        target.width = 800;
        target.height = 400;
      },
      getTextMap: async () => [
        { text, x: 0, y: 0, width: 760, height: 200, direction: "ltr" },
      ],
      close: () => {},
    };
    const client = ViewerClient.create({
      adapters: [adapter],
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer();
    const warnings: string[] = [];
    viewer.on("warning", (warning) => warnings.push(warning.code));
    await viewer.load(new TextEncoder().encode("%PDF-1.7\nlanguages"), {
      fileName: "languages.pdf",
    });
    await viewer.renderPage(0, document.createElement("canvas"));
    await viewer.destroy();
    await client.destroy();
    return warnings;
  });

  expect([...fontRequests].sort()).toEqual(
    [
      "arabic",
      "bengali",
      "cjk",
      "devanagari",
      "gujarati",
      "gurmukhi",
      "japanese",
      "kannada",
      "korean",
      "malayalam",
      "odia",
      "tamil",
      "telugu",
    ].map((script) => `/fonts/noto-sans-${script}.woff2`),
  );
  expect(warnings).toEqual([]);
});

test("parses and renders DOCX with the qualified OOXML engine", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const startedAt = performance.now();
    const loadModule = new Function("url", "return import(url)") as (
      url: string,
    ) => Promise<{
      DocxDocument: {
        load(source: ArrayBuffer): Promise<{
          pageCount: number;
          renderPage(
            canvas: HTMLCanvasElement,
            pageIndex: number,
          ): Promise<void>;
          destroy(): void;
        }>;
      };
    }>;
    const [{ DocxDocument }, response] = await Promise.all([
      loadModule("/vendor/ooxml/docx.mjs"),
      fetch("/corpus/sample.docx"),
    ]);
    const document = await DocxDocument.load(await response.arrayBuffer());
    const canvas = window.document.createElement("canvas");
    canvas.dataset.testid = "docx-canvas";
    Object.assign(canvas.style, {
      display: "block",
      left: "0",
      position: "absolute",
      top: "0",
    });
    window.document.body.append(canvas);
    await document.renderPage(canvas, 0);
    const output = {
      elapsedMs: performance.now() - startedAt,
      pageCount: document.pageCount,
      width: canvas.width,
      height: canvas.height,
    };
    document.destroy();
    return output;
  });

  expect(result.pageCount).toBeGreaterThan(0);
  expect(result.width).toBeGreaterThan(0);
  expect(result.height).toBeGreaterThan(0);
  await expect(page.getByTestId("docx-canvas")).toHaveScreenshot(
    "docx-sample.png",
  );
  console.log(
    "QUALIFICATION_METRIC",
    JSON.stringify({ adapter: "docx", ...result }),
  );
});

test("routes modern Office through the package default adapter", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as {
      ViewerClient: {
        create(options: { assetBaseUrl: URL }): {
          createViewer(): {
            load(
              source: Uint8Array,
              options: { fileName: string },
            ): Promise<void>;
            readonly state: {
              status: string;
              format?: string;
              pageCount: number;
            };
            destroy(): Promise<void>;
          };
          destroy(): Promise<void>;
        };
      };
    };
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const output = [];
    for (const fileName of ["sample.docx", "sample.xlsx", "sample.pptx"]) {
      const viewer = client.createViewer();
      const response = await fetch(`/corpus/${fileName}`);
      await viewer.load(new Uint8Array(await response.arrayBuffer()), {
        fileName,
      });
      output.push({ ...viewer.state });
      await viewer.destroy();
    }
    await client.destroy();
    return output;
  });

  expect(result.map((state) => state.format)).toEqual(["docx", "xlsx", "pptx"]);
  for (const state of result) {
    expect(state.status).toBe("ready");
    expect(state.pageCount).toBeGreaterThan(0);
  }
});

test("converts legacy Office in the package worker and opens the result", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as {
      ViewerClient: {
        create(options: { assetBaseUrl: URL }): {
          createViewer(): {
            load(
              source: Uint8Array,
              options: { fileName: string },
            ): Promise<void>;
            readonly state: {
              status: string;
              format?: string;
              pageCount: number;
            };
          };
          destroy(): Promise<void>;
        };
      };
    };
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const output = [];
    for (const fileName of ["word6.doc", "simple.xls", "basic.ppt"]) {
      const viewer = client.createViewer();
      const response = await fetch(`/corpus/${fileName}`);
      await viewer.load(new Uint8Array(await response.arrayBuffer()), {
        fileName,
      });
      output.push({ ...viewer.state });
      await viewer.destroy();
    }
    await client.destroy();
    return output;
  });

  expect(result.map((state) => state.format)).toEqual(["doc", "xls", "ppt"]);
  for (const state of result) {
    expect(state.status).toBe("ready");
    expect(state.pageCount).toBeGreaterThan(0);
  }
});

test("opens PDF, raster, TIFF, SVG, and delimited data through built-in adapters", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as {
      ViewerClient: {
        create(options: { assetBaseUrl: URL }): {
          createViewer(): {
            load(
              source: Uint8Array,
              options: { fileName: string },
            ): Promise<void>;
            readonly state: {
              status: string;
              format?: string;
              pageCount: number;
            };
            destroy(): Promise<void>;
          };
          destroy(): Promise<void>;
        };
      };
    };
    const fromBase64 = (value: string): Uint8Array =>
      Uint8Array.from(atob(value), (character) => character.charCodeAt(0));
    const pdf = new Uint8Array(
      await (await fetch("/corpus/hello.pdf")).arrayBuffer(),
    );
    const cases = [
      { fileName: "hello.pdf", data: pdf },
      {
        fileName: "pixel.png",
        data: fromBase64(
          "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Zl1sAAAAASUVORK5CYII=",
        ),
      },
      {
        fileName: "page.tiff",
        data: fromBase64(
          "SUkqAA4AAAD//wAAAAAPAAABAwABAAAAAQAAAAEBAwABAAAAAQAAAAIBAwADAAAAyAAAAAMBAwABAAAAAQAAAAYBAwABAAAAAgAAAAoBAwABAAAAAQAAABEBBAABAAAACAAAABIBAwABAAAAAQAAABUBAwABAAAAAwAAABYBAwABAAAAAQAAABcBBAABAAAABgAAABwBAwABAAAAAQAAACkBAwACAAAAAAABAD4BBQACAAAA/gAAAD8BBQAGAAAAzgAAAAAAAAAQABAAEACF61EAAACAAMP1qAAAAAACzcxMAAAAAAHNzEwAAACAAM3MTAAAAAACj8L1AAAAABA3GqAAAAAAAiuHCgAAACAA",
        ),
      },
      {
        fileName: "safe.svg",
        data: new TextEncoder().encode(
          '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><script/><rect width="10" height="10"/></svg>',
        ),
      },
      {
        fileName: "data.csv",
        data: new TextEncoder().encode("язык,текст\nالعربية,नमस्ते"),
      },
      {
        fileName: "data.tsv",
        data: new TextEncoder().encode("key\tvalue\n漢字\t한글"),
      },
    ];
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const output = [];
    for (const fixture of cases) {
      const viewer = client.createViewer();
      await viewer.load(fixture.data, { fileName: fixture.fileName });
      output.push({ ...viewer.state });
      await viewer.destroy();
    }
    await client.destroy();
    return output;
  });

  expect(result.map((state) => state.format)).toEqual([
    "pdf",
    "png",
    "tiff",
    "svg",
    "csv",
    "tsv",
  ]);
  for (const state of result) {
    expect(state.status).toBe("ready");
    expect(state.pageCount).toBeGreaterThan(0);
  }
});

test("converts legacy DOC to OOXML in browser WASM", async ({ page }) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const loadModule = new Function("url", "return import(url)") as (
      url: string,
    ) => Promise<{
      default(): Promise<void>;
      convertLegacyToOoxml(data: Uint8Array, format: string): Uint8Array;
    }>;
    const [module, response] = await Promise.all([
      loadModule("/wasm/legacy/index.js"),
      fetch("/corpus/word6.doc"),
    ]);
    await module.default();
    const startedAt = performance.now();
    const output = module.convertLegacyToOoxml(
      new Uint8Array(await response.arrayBuffer()),
      "doc",
    );
    return {
      elapsedMs: performance.now() - startedAt,
      size: output.byteLength,
      signature: Array.from(output.slice(0, 4)),
    };
  });

  expect(result.signature).toEqual([0x50, 0x4b, 0x03, 0x04]);
  expect(result.size).toBeGreaterThan(500);
  console.log(
    "QUALIFICATION_METRIC",
    JSON.stringify({ adapter: "legacy-doc", ...result }),
  );
});

test("renders PDF and extracts its text map in browser WASM", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const loadModule = new Function("url", "return import(url)") as (
      url: string,
    ) => Promise<{
      default(): Promise<void>;
      PdfViewerDocument: new (data: Uint8Array) => {
        pageCount(): number;
        renderPagePng(pageIndex: number, dpi?: number): Uint8Array;
        pageTextJson(pageIndex: number): string;
        free(): void;
      };
    }>;
    const [module, response] = await Promise.all([
      loadModule("/wasm/pdf/index.js"),
      fetch("/corpus/hello.pdf"),
    ]);
    await module.default();
    const startedAt = performance.now();
    const document = new module.PdfViewerDocument(
      new Uint8Array(await response.arrayBuffer()),
    );
    const pageCount = document.pageCount();
    const png = document.renderPagePng(0, 72);
    const text = JSON.parse(document.pageTextJson(0)) as { chars: unknown[] };
    document.free();
    return {
      elapsedMs: performance.now() - startedAt,
      pageCount,
      pngSize: png.byteLength,
      pngSignature: Array.from(png.slice(0, 8)),
      characterCount: text.chars.length,
    };
  });

  expect(result.pageCount).toBe(1);
  expect(result.pngSignature).toEqual([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
  ]);
  expect(result.pngSize).toBeGreaterThan(100);
  expect(result.characterCount).toBeGreaterThan(0);
  console.log(
    "QUALIFICATION_METRIC",
    JSON.stringify({ adapter: "pdf", ...result }),
  );
});
