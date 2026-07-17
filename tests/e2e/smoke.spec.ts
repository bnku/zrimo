import { expect, test } from "@playwright/test";

test("loads the ESM package in Chromium", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Viewer status: idle")).toBeVisible();
});

test("vanilla example stretches the viewer through the available height", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/?demo=1");

  const layout = await page.locator("#viewer").evaluate((host) => {
    const root = host.querySelector<HTMLElement>(".docs-viewer-ui");
    const hostRect = host.getBoundingClientRect();
    const rootRect = root?.getBoundingClientRect();
    return {
      viewportHeight: window.innerHeight,
      hostHeight: hostRect.height,
      hostBottom: hostRect.bottom,
      rootHeight: rootRect?.height ?? 0,
      rootBottom: rootRect?.bottom ?? 0,
    };
  });

  expect(layout.hostHeight).toBeGreaterThan(layout.viewportHeight * 0.7);
  expect(layout.rootHeight).toBeCloseTo(layout.hostHeight, 0);
  expect(layout.hostBottom).toBeCloseTo(layout.viewportHeight, 0);
  expect(layout.rootBottom).toBeCloseTo(layout.viewportHeight, 0);
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

test("keeps heterogeneous PDF page geometry stable and centered", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as any;
    const pageSizes = [
      { width: 600, height: 900 },
      { width: 900, height: 500 },
      { width: 700, height: 1_200 },
    ];
    const adapter = {
      id: "e2e-variable-pages",
      formats: ["pdf"],
      open: async () => ({}),
      getInfo: async () => ({
        format: "pdf",
        unit: "page",
        pageCount: pageSizes.length,
        pageSizes,
      }),
      render: async (
        _handle: unknown,
        target: HTMLCanvasElement,
        viewport: { pageIndex: number; zoom: number; devicePixelRatio: number },
      ) => {
        const size = pageSizes[viewport.pageIndex]!;
        target.width = size.width * viewport.zoom * viewport.devicePixelRatio;
        target.height = size.height * viewport.zoom * viewport.devicePixelRatio;
      },
      getTextMap: async () => [],
      close: () => {},
    };
    const container = document.createElement("div");
    Object.assign(container.style, { width: "800px", height: "600px" });
    document.body.append(container);
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container });
    await viewer.load(new TextEncoder().encode("%PDF-1.7\nvariable"), {
      fileName: "variable.pdf",
    });
    const settle = () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      );
    await settle();
    const root = container.querySelector<HTMLElement>(
      '[data-docs-viewer="viewport"]',
    )!;
    const first = root.querySelector<HTMLElement>('[data-page-index="0"]')!;
    const initial = {
      left: first.offsetLeft,
      width: first.getBoundingClientRect().width,
      scrollWidth: root.scrollWidth,
    };
    root.scrollTo({ top: 924 });
    await settle();
    const second = root.querySelector<HTMLElement>('[data-page-index="1"]')!;
    const middle = {
      left: second.offsetLeft,
      width: second.getBoundingClientRect().width,
      scrollWidth: root.scrollWidth,
    };
    root.scrollTo({ top: 0 });
    await settle();
    const firstAgain = root.querySelector<HTMLElement>(
      '[data-page-index="0"]',
    )!;
    const final = {
      left: firstAgain.offsetLeft,
      width: firstAgain.getBoundingClientRect().width,
      scrollWidth: root.scrollWidth,
    };
    await viewer.destroy();
    await client.destroy();
    container.remove();
    return { initial, middle, final };
  });

  expect(result.initial.width).toBe(600);
  expect(result.middle.width).toBe(900);
  expect(result.final).toEqual(result.initial);
  expect(result.initial.left).toBeGreaterThan(100);
  expect(result.middle.left).toBeGreaterThanOrEqual(12);
  expect(result.middle.scrollWidth).toBe(result.initial.scrollWidth);
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
            defaultColumnWidth: 120,
            defaultRowHeight: 50,
            rowHeaderWidth: 50,
            columnHeaderHeight: 22,
          },
        ],
      }),
      render: async (
        _handle: unknown,
        target: HTMLCanvasElement,
        viewport: {
          width: number;
          height: number;
          devicePixelRatio: number;
          sheetRange: {
            row: number;
            column: number;
            rowCount: number;
            columnCount: number;
          };
        },
      ) => {
        renderedRanges.push(viewport.sheetRange);
        target.width = viewport.width * viewport.devicePixelRatio;
        target.height = viewport.height * viewport.devicePixelRatio;
      },
      getTextMap: async () => cells,
      close: () => {},
    };
    const renderedRanges: Array<{
      row: number;
      column: number;
      rowCount: number;
      columnCount: number;
    }> = [];
    const container = document.createElement("div");
    Object.assign(container.style, { width: "800px", height: "600px" });
    document.body.append(container);
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container });
    let shortcutCopies = 0;
    const originalCopySelection = viewer.copySelection.bind(viewer);
    viewer.copySelection = async () => {
      shortcutCopies += 1;
      return originalCopySelection();
    };
    await viewer.load(new TextEncoder().encode("A,B\nC,D"), {
      fileName: "cells.csv",
    });
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
    const root = container.querySelector<HTMLElement>(
      '[data-docs-viewer="spreadsheet-viewport"]',
    )!;
    const surface = root.querySelector<HTMLElement>(
      '[data-docs-viewer-layer="cell-selection"]',
    )!;
    const bounds = root.getBoundingClientRect();
    const start = { x: bounds.x + 55, y: bounds.y + 27 };
    const end = { x: bounds.x + 175, y: bounds.y + 77 };
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
    pointer(surface, "pointerdown", start.x, start.y, 1);
    pointer(surface, "pointermove", end.x, end.y, 1);
    pointer(surface, "pointerup", end.x, end.y, 0);
    const dragged = viewer.getSelection();

    pointer(surface, "pointerdown", start.x, start.y, 1);
    pointer(surface, "pointerup", start.x, start.y, 0);
    root.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", shiftKey: true }),
    );
    root.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", shiftKey: true }),
    );
    const keyboard = viewer.getSelection();
    const copied = await viewer.copySelection();
    root.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "c",
        ctrlKey: true,
        bubbles: true,
      }),
    );
    await new Promise<void>((resolve) => queueMicrotask(resolve));
    await viewer.destroy();
    await client.destroy();
    container.remove();
    return {
      dragged,
      keyboard,
      copied,
      shortcutCopies,
      renderedRange: renderedRanges.at(-1),
    };
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
  expect(result.shortcutCopies).toBe(2);
  expect(
    (result.renderedRange?.column ?? 0) +
      (result.renderedRange?.columnCount ?? 0) -
      1,
  ).toBeGreaterThan(2);
  expect(
    (result.renderedRange?.row ?? 0) +
      (result.renderedRange?.rowCount ?? 0) -
      1,
  ).toBeGreaterThan(2);
});

test("virtualizes the full spreadsheet used range at every supported zoom", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as any;
    const renders: Array<{
      row: number;
      column: number;
      rowCount: number;
      columnCount: number;
      offsetX: number;
      offsetY: number;
    }> = [];
    const maxRow = 10_000;
    const maxColumn = 1_000;
    const adapter = {
      id: "e2e-sheet-virtualization",
      formats: ["csv"],
      open: async () => ({}),
      getInfo: async () => ({
        format: "csv",
        unit: "sheet",
        pageCount: 1,
        sheetNames: ["Sparse"],
        sheets: [
          {
            name: "Sparse",
            frozenRows: 2,
            frozenColumns: 2,
            mergedRanges: [
              {
                startRow: 9_998,
                startColumn: 998,
                endRow: maxRow,
                endColumn: maxColumn,
              },
            ],
            maxRow,
            maxColumn,
            defaultColumnWidth: 80,
            defaultRowHeight: 24,
            columnWidths: { 3: 0, 500: 160, 999: 32 },
            rowHeights: { 3: 0, 5_000: 48, 9_999: 12 },
            rowHeaderWidth: 50,
            columnHeaderHeight: 22,
          },
        ],
      }),
      render: async (
        _handle: unknown,
        target: HTMLCanvasElement,
        viewport: {
          sheetRange: {
            row: number;
            column: number;
            rowCount: number;
            columnCount: number;
          };
          width: number;
          height: number;
          devicePixelRatio: number;
          scrollOffsetX?: number;
          scrollOffsetY?: number;
        },
      ) => {
        const range = viewport.sheetRange;
        renders.push({
          ...range,
          offsetX: viewport.scrollOffsetX ?? 0,
          offsetY: viewport.scrollOffsetY ?? 0,
        });
        const isDeep = range.row > maxRow / 2;
        await new Promise((resolve) => setTimeout(resolve, isDeep ? 5 : 70));
        target.width = Math.ceil(viewport.width * viewport.devicePixelRatio);
        target.height = Math.ceil(viewport.height * viewport.devicePixelRatio);
        const context = target.getContext("2d")!;
        context.fillStyle = isDeep ? "#00ff00" : "#ff0000";
        context.fillRect(0, 0, target.width, target.height);
      },
      getTextMap: async () => [
        {
          text: "origin",
          x: 54,
          y: 24,
          width: 70,
          height: 20,
          row: 1,
          column: 1,
        },
        {
          text: "last",
          x: 0,
          y: 0,
          width: 32,
          height: 24,
          row: maxRow,
          column: maxColumn,
        },
      ],
      close: () => {},
    };
    const container = document.createElement("div");
    Object.assign(container.style, { width: "720px", height: "480px" });
    document.body.append(container);
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container });
    await viewer.load(new TextEncoder().encode("synthetic"), {
      fileName: "sparse.csv",
    });
    const root = container.querySelector<HTMLElement>(
      '[data-docs-viewer="spreadsheet-viewport"]',
    )!;
    const settle = async (delay = 0) => {
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      );
      if (delay) await new Promise((resolve) => setTimeout(resolve, delay));
    };
    const zoomResults: Array<{
      zoom: number;
      lastRow: number;
      lastColumn: number;
      scrollWidth: number;
      scrollHeight: number;
    }> = [];
    for (const zoom of [0.25, 0.5, 1, 2, 4]) {
      viewer.setZoom(zoom);
      await settle();
      root.scrollTo({ left: root.scrollWidth, top: root.scrollHeight });
      await settle(15);
      const frame = renders[renders.length - 1]!;
      zoomResults.push({
        zoom,
        lastRow: frame.row + frame.rowCount - 1,
        lastColumn: frame.column + frame.columnCount - 1,
        scrollWidth: root.scrollWidth,
        scrollHeight: root.scrollHeight,
      });
    }
    root.scrollTo({ left: 0, top: 0 });
    viewer.setZoom(1);
    await settle();
    root.scrollTo({ left: root.scrollWidth, top: root.scrollHeight });
    await settle(100);
    const canvas = root.querySelector<HTMLCanvasElement>(
      '[data-docs-viewer-layer="spreadsheet-canvas"]',
    )!;
    const pixel = Array.from(
      canvas.getContext("2d")!.getImageData(0, 0, 1, 1).data,
    );
    const surface = root.querySelector<HTMLElement>(
      '[data-docs-viewer-layer="cell-selection"]',
    )!;
    const bounds = root.getBoundingClientRect();
    const pointer = (type: string, x: number, y: number, buttons: number) =>
      surface.dispatchEvent(
        new PointerEvent(type, {
          pointerId: 44,
          pointerType: "mouse",
          clientX: x,
          clientY: y,
          button: 0,
          buttons,
          bubbles: true,
        }),
      );
    pointer("pointerdown", bounds.right - 20, bounds.bottom - 20, 1);
    pointer("pointerup", bounds.right - 20, bounds.bottom - 20, 0);
    root.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowLeft", shiftKey: true }),
    );
    const selection = viewer.getSelection();
    const metrics = {
      canvasCount: root.querySelectorAll("canvas").length,
      elementCount: root.querySelectorAll("*").length,
      canvasCssWidth: canvas.getBoundingClientRect().width,
      canvasCssHeight: canvas.getBoundingClientRect().height,
      rootWidth: root.clientWidth,
      rootHeight: root.clientHeight,
      spacerWidth: root.firstElementChild?.getBoundingClientRect().width ?? 0,
      spacerHeight: root.firstElementChild?.getBoundingClientRect().height ?? 0,
    };
    await viewer.destroy();
    await client.destroy();
    container.remove();
    return { zoomResults, pixel, selection, metrics };
  });

  for (const entry of result.zoomResults) {
    expect(entry.lastRow).toBe(10_000);
    expect(entry.lastColumn).toBe(1_000);
    expect(entry.scrollWidth).toBeGreaterThan(816);
    expect(entry.scrollHeight).toBeGreaterThan(1056);
  }
  expect(result.pixel.slice(0, 3)).toEqual([0, 255, 0]);
  expect(result.selection).toMatchObject({
    sheetIndex: 0,
    startColumn: 998,
    endRow: 10_000,
    endColumn: 1_000,
  });
  expect(result.metrics.canvasCount).toBe(1);
  expect(result.metrics.elementCount).toBeLessThan(10);
  expect(result.metrics.canvasCssWidth).toBe(result.metrics.rootWidth);
  expect(result.metrics.canvasCssHeight).toBe(result.metrics.rootHeight);
  expect(result.metrics.spacerWidth).not.toBeCloseTo(816, 0);
  expect(result.metrics.spacerHeight).not.toBeCloseTo(1056, 0);
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
    for (const fileName of ["simple.xls", "basic.ppt"]) {
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

  expect(result.map((state) => state.format)).toEqual(["xls", "ppt"]);
  for (const state of result) {
    expect(state.status).toBe("ready");
    expect(state.pageCount).toBeGreaterThan(0);
  }
});

test("converts and renders Word 97 DOC through the package adapter", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = await import("/main.js");
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer();
    const response = await fetch("/corpus/word97-simple-table.doc");
    try {
      await viewer.load(new Uint8Array(await response.arrayBuffer()), {
        fileName: "word97-simple-table.doc",
      });
      const canvas = document.createElement("canvas");
      await viewer.renderPage(0, canvas, {
        zoom: 1,
        devicePixelRatio: 1,
      });
      const text = await viewer.getPageText(0);
      const token = text.match(/[A-Za-z]{3,}/u)?.[0] ?? text.trim().slice(0, 3);
      const tokenStart = text.indexOf(token);
      const search = await viewer.search(token);
      const selection = await viewer.selectText({
        startPageIndex: 0,
        startOffset: tokenStart,
        endPageIndex: 0,
        endOffset: tokenStart + token.length,
      });
      const copied = await viewer.copySelection();
      viewer.setZoom(1.5);
      viewer.panBy(24, 32);
      return {
        state: { ...viewer.state },
        text,
        token,
        searchMatches: search.matches.length,
        selection: selection.text,
        copied,
        canvas: { width: canvas.width, height: canvas.height },
      };
    } finally {
      await viewer.destroy();
      await client.destroy();
    }
  });

  expect(result.state.status).toBe("ready");
  expect(result.state.format).toBe("doc");
  expect(result.state.pageCount).toBeGreaterThan(0);
  expect(result.state.zoom).toBe(1.5);
  expect(result.state.panX).toBe(24);
  expect(result.state.panY).toBe(32);
  expect(result.text.trim().length).toBeGreaterThan(0);
  expect(result.searchMatches).toBeGreaterThan(0);
  expect(result.selection).toBe(result.token);
  expect(result.copied).toBe(result.token);
  expect(result.canvas.width).toBeGreaterThan(0);
  expect(result.canvas.height).toBeGreaterThan(0);
});

test("cancels and bounds Word 97 DOC conversion in the package worker", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = await import("/main.js");
    const response = await fetch("/corpus/word97-simple-table.doc");
    const bytes = new Uint8Array(await response.arrayBuffer());
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer();
    const errorCode = (error: unknown): string =>
      typeof error === "object" && error !== null && "code" in error
        ? String(error.code)
        : "unknown";
    try {
      const controller = new AbortController();
      const pending = viewer.load(bytes.slice(), {
        fileName: "word97-simple-table.doc",
        signal: controller.signal,
      });
      controller.abort();
      let abortCode = "completed";
      try {
        await pending;
      } catch (error) {
        abortCode = errorCode(error);
      }

      let limitCode = "completed";
      try {
        await viewer.load(bytes.slice(), {
          fileName: "word97-simple-table.doc",
          limits: { maxInputBytes: bytes.byteLength - 1 },
        });
      } catch (error) {
        limitCode = errorCode(error);
      }

      await viewer.load(bytes.slice(), {
        fileName: "word97-simple-table.doc",
      });
      return {
        abortCode,
        limitCode,
        recovered: viewer.state.status,
      };
    } finally {
      await viewer.destroy();
      await client.destroy();
    }
  });

  expect(result.abortCode).toBe("aborted");
  expect(result.limitCode).toBe("resource-limit");
  expect(result.recovered).toBe("ready");
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

test("converts and renders Word 97 comments and numbering in browser WASM", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = await import("/main.js");
    const loadModule = new Function("url", "return import(url)") as (
      url: string,
    ) => Promise<{
      default(): Promise<void>;
      convertLegacyToOoxml(data: Uint8Array, format: string): Uint8Array;
    }>;
    const [module, response] = await Promise.all([
      loadModule("/wasm/legacy/index.js"),
      fetch("/corpus/word97-comments.doc"),
    ]);
    await module.default();
    const source = new Uint8Array(await response.arrayBuffer());
    const output = module.convertLegacyToOoxml(source, "doc");
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer();
    try {
      await viewer.load(source, { fileName: "word97-comments.doc" });
      const canvas = document.createElement("canvas");
      await viewer.renderPage(0, canvas, {
        zoom: 1,
        devicePixelRatio: 1,
      });
      const text = await viewer.getPageText(0);
      return {
        byteLength: output.byteLength,
        signature: Array.from(output.subarray(0, 4)),
        state: { ...viewer.state },
        text,
        canvas: { width: canvas.width, height: canvas.height },
      };
    } finally {
      await viewer.destroy();
      await client.destroy();
    }
  });

  expect(result.byteLength).toBeGreaterThan(100);
  expect(result.signature).toEqual([0x50, 0x4b, 0x03, 0x04]);
  expect(result.state.status).toBe("ready");
  expect(result.state.format).toBe("doc");
  expect(result.state.pageCount).toBeGreaterThan(0);
  expect(result.text).toContain("The Lifecycle of a Project");
  expect(result.canvas.width).toBeGreaterThan(0);
  expect(result.canvas.height).toBeGreaterThan(0);
});

test("converts and renders a Word 97 ranged-comment fixture in browser WASM", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = await import("/main.js");
    const response = await fetch("/corpus/word97-ranged-comment.doc");
    const source = new Uint8Array(await response.arrayBuffer());
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer();
    try {
      await viewer.load(source, { fileName: "word97-ranged-comment.doc" });
      const canvas = document.createElement("canvas");
      await viewer.renderPage(0, canvas, {
        zoom: 1,
        devicePixelRatio: 1,
      });
      const pages = await Promise.all(
        Array.from({ length: viewer.state.pageCount }, (_, pageIndex) =>
          viewer.getPageText(pageIndex),
        ),
      );
      return {
        state: { ...viewer.state },
        text: pages.join("\n"),
        canvas: { width: canvas.width, height: canvas.height },
      };
    } finally {
      await viewer.destroy();
      await client.destroy();
    }
  });

  expect(result.state.status).toBe("ready");
  expect(result.state.format).toBe("doc");
  expect(result.state.pageCount).toBeGreaterThan(0);
  expect(result.text).toContain("There is a comment");
  expect(result.text).toContain("文京区と目黒区");
  expect(result.canvas.width).toBeGreaterThan(0);
  expect(result.canvas.height).toBeGreaterThan(0);
});

test("renders PDF and extracts its text map through self-hosted PDF.js", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as any;
    performance.clearResourceTimings();
    const response = await fetch("/corpus/hello.pdf");
    const bytes = new Uint8Array(await response.arrayBuffer());
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const container = document.createElement("div");
    Object.assign(container.style, { width: "700px", height: "820px" });
    document.body.append(container);
    const viewer = client.createViewer({ container });
    const startedAt = performance.now();
    await viewer.load(bytes, { fileName: "hello.pdf" });
    const pageCount = viewer.state.pageCount;
    const canvas = document.createElement("canvas");
    await viewer.renderPage(0, canvas, { zoom: 1, devicePixelRatio: 1 });
    const text = await viewer.getPageText(0);
    const context = canvas.getContext("2d")!;
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    let darkPixels = 0;
    for (let index = 0; index < pixels.length; index += 4)
      if (
        pixels[index]! < 230 ||
        pixels[index + 1]! < 230 ||
        pixels[index + 2]! < 230
      )
        darkPixels += 1;
    const resources = performance
      .getEntriesByType("resource")
      .map((entry) => new URL(entry.name))
      .filter(
        (url) => url.pathname.includes("pdf") || url.pathname.includes("cmaps"),
      );
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
    const spans = Array.from(
      container.querySelectorAll<HTMLElement>(
        '[data-docs-viewer-layer="text"] [data-start]',
      ),
    );
    const attachedCanvas = container.querySelector("canvas")!;
    const attachedBounds = attachedCanvas.getBoundingClientRect();
    const overlayInsideCanvas = spans.every((span) => {
      const bounds = span.getBoundingClientRect();
      return (
        bounds.left >= attachedBounds.left - 1 &&
        bounds.top >= attachedBounds.top - 1 &&
        bounds.right <= attachedBounds.right + 1 &&
        bounds.bottom <= attachedBounds.bottom + 1
      );
    });
    const firstText = spans[0]?.firstChild;
    const lastText = spans.at(-1)?.firstChild;
    if (firstText && lastText) {
      const range = document.createRange();
      range.setStart(firstText, 0);
      range.setEnd(lastText, lastText.textContent?.length ?? 0);
      const selection = document.getSelection()!;
      selection.removeAllRanges();
      selection.addRange(range);
      document.dispatchEvent(new Event("selectionchange"));
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    const copied = await viewer.copySelection();
    await viewer.destroy();
    await client.destroy();
    container.remove();
    return {
      elapsedMs: performance.now() - startedAt,
      pageCount,
      width: canvas.width,
      height: canvas.height,
      text,
      characterCount: text.length,
      darkPixels,
      resourceHosts: resources.map((url) => url.host),
      resourcePaths: resources.map((url) => url.pathname),
      spanCount: spans.length,
      overlayInsideCanvas,
      copied,
    };
  });

  expect(result.width).toBe(612);
  expect(result.height).toBe(792);
  expect(result.pageCount).toBe(1);
  expect(result.characterCount).toBeGreaterThan(0);
  expect(result.darkPixels).toBeGreaterThan(100);
  expect(result.spanCount).toBeGreaterThan(0);
  expect(result.overlayInsideCanvas).toBe(true);
  expect(result.copied).toBe(result.text);
  expect(new Set(result.resourceHosts)).toEqual(new Set(["127.0.0.1:4173"]));
  expect(result.resourcePaths).toContain("/workers/pdf.worker.min.mjs");
  console.log(
    "QUALIFICATION_METRIC",
    JSON.stringify({ adapter: "pdf", ...result }),
  );
});
