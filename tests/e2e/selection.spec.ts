import { expect, test } from "@playwright/test";

const logicalText = "Hello Привет 日本語 العربية हिन्दी";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate(
    async ({ logicalText }) => {
      const { ViewerClient } = await import("/main.js");
      const sourceRuns = [
        { text: "Hello ", x: 24, y: 36, font: "20px sans-serif" },
        {
          text: "Привет ",
          x: 90,
          y: 36,
          font: "700 20px sans-serif",
          letterSpacingPx: 0.4,
        },
        { text: "日本語 ", x: 188, y: 36, font: "20px sans-serif" },
        {
          text: "العربية ",
          x: 24,
          y: 76,
          font: "20px sans-serif",
          direction: "rtl" as const,
        },
        { text: "हिन्दी", x: 118, y: 76, font: "20px sans-serif" },
        {
          text: "12",
          x: 250,
          y: 76,
          font: "20px sans-serif",
          transform: "rotate(90deg)",
          eastAsianVert: true,
        },
      ];
      const measure = document.createElement("canvas").getContext("2d")!;
      let offset = 0;
      const runs = sourceRuns.map((run) => {
        measure.font = run.font;
        const logicalStart = offset;
        offset += run.text.length;
        return {
          ...run,
          width:
            measure.measureText(run.text).width +
            (run.letterSpacingPx ?? 0) * run.text.length,
          height: 24,
          fontSize: 20,
          textLayer: "docx" as const,
          coordinateWidth: 420,
          coordinateHeight: 180,
          logicalStart,
          logicalEnd: offset,
        };
      });
      const adapter = {
        id: "docx-selection-fixture",
        formats: ["docx"] as const,
        open: async () => ({}),
        getInfo: async () =>
          ({ format: "docx", unit: "page", pageCount: 1 }) as const,
        render: async (
          _handle: unknown,
          canvas: HTMLCanvasElement,
          viewport: { zoom: number; devicePixelRatio: number },
        ) => {
          const scale = viewport.zoom * viewport.devicePixelRatio;
          canvas.width = Math.round(420 * scale);
          canvas.height = Math.round(180 * scale);
          const context = canvas.getContext("2d")!;
          context.scale(scale, scale);
          context.textBaseline = "top";
          for (const run of runs) {
            context.font = run.font;
            context.direction = run.direction ?? "ltr";
            if ("letterSpacing" in context)
              context.letterSpacing = `${run.letterSpacingPx ?? 0}px`;
            context.fillText(run.text, run.x, run.y);
          }
        },
        getTextMap: async () => runs,
        close: () => {},
      };
      const host = document.createElement("div");
      host.dataset.testid = "selection-host";
      Object.assign(host.style, { width: "700px", height: "480px" });
      document.body.replaceChildren(host);
      const client = ViewerClient.create({ adapters: [adapter] });
      const viewer = client.createViewer({ container: host, initialZoom: 1 });
      const fixture = new Uint8Array(
        await (await fetch("/corpus/sample.docx")).arrayBuffer(),
      );
      await viewer.load(fixture, {
        fileName: "fixture.docx",
      });
      Object.assign(window, {
        __selectionClient: client,
        __selectionViewer: viewer,
        __selectionRuns: runs,
        __selectionLogicalText: logicalText,
      });
    },
    { logicalText },
  );
  await expect(page.locator("[data-start]")).toHaveCount(6);
});

test.afterEach(async ({ page }) => {
  await page.evaluate(async () => {
    const state = window as Window & {
      __selectionClient?: { destroy(): Promise<void> };
      __selectionViewer?: { destroy(): Promise<void> };
    };
    await state.__selectionViewer?.destroy();
    await state.__selectionClient?.destroy();
  });
});

test("DOCX overlay tracks glyph geometry and zoom without giant rectangles", async ({
  page,
}) => {
  for (const zoom of [0.5, 1, 2]) {
    await page.evaluate((value) => {
      (
        window as Window & {
          __selectionViewer: { setZoom(zoom: number): void };
        }
      ).__selectionViewer.setZoom(value);
    }, zoom);
    await expect(page.locator('[data-zrimo-layer="text"]')).toBeAttached();
    await page.waitForFunction(
      (value) =>
        document.querySelector<HTMLElement>('[data-zrimo-layer="text"]')?.style
          .transform === `scale(${value})`,
      zoom,
    );
    const result = await page.evaluate(() => {
      const state = window as Window & {
        __selectionRuns: Array<{
          x: number;
          y: number;
          width: number;
          height: number;
          font: string;
          letterSpacingPx?: number;
        }>;
        __selectionViewer: { state: { zoom: number } };
      };
      const pageRoot =
        document.querySelector<HTMLElement>("[data-page-index]")!;
      const rootRect = pageRoot.getBoundingClientRect();
      const spans = [...document.querySelectorAll<HTMLElement>("[data-start]")];
      const overlaps = spans.slice(0, 5).map((span, index) => {
        const run = state.__selectionRuns[index]!;
        const rect = span.getBoundingClientRect();
        const expected = {
          left: rootRect.left + run.x * state.__selectionViewer.state.zoom,
          top: rootRect.top + run.y * state.__selectionViewer.state.zoom,
          right:
            rootRect.left +
            (run.x + run.width) * state.__selectionViewer.state.zoom,
          bottom:
            rootRect.top +
            (run.y + run.height) * state.__selectionViewer.state.zoom,
        };
        const intersection =
          Math.max(
            0,
            Math.min(rect.right, expected.right) -
              Math.max(rect.left, expected.left),
          ) *
          Math.max(
            0,
            Math.min(rect.bottom, expected.bottom) -
              Math.max(rect.top, expected.top),
          );
        const expectedArea =
          (expected.right - expected.left) * (expected.bottom - expected.top);
        return intersection / Math.max(1, expectedArea);
      });
      return {
        overlaps,
        fonts: spans.map((span) => span.style.font),
        letterSpacing: spans[1]?.style.letterSpacing,
        verticalTransform: spans[5]?.style.transform,
        maximumWidth: Math.max(
          ...spans.map((span) => span.getBoundingClientRect().width),
        ),
        pageWidth: rootRect.width,
      };
    });
    expect(Math.min(...result.overlaps)).toBeGreaterThanOrEqual(0.9);
    expect(result.maximumWidth).toBeLessThan(result.pageWidth / 2);
    expect(result.fonts[1]).toContain("700");
    expect(result.letterSpacing).not.toBe("");
    expect(result.verticalTransform).toContain("rotate(90deg)");
  }

  await page.locator('[data-testid="selection-host"]').evaluate((host) => {
    (host as HTMLElement).style.width = "520px";
  });
  await expect(page.locator("[data-start]")).toHaveCount(6);
  await expect(page.locator('[data-zrimo-layer="text"]')).toHaveCSS(
    "transform",
    /matrix\(2, 0, 0, 2/,
  );
});

test("native cross-run selection preserves logical text and grapheme boundaries", async ({
  page,
}) => {
  const result = await page.evaluate(async () => {
    const state = window as Window & {
      __selectionViewer: {
        getSelection(): {
          text: string;
          startOffset?: number;
          endOffset?: number;
        } | null;
        copySelection(): Promise<string>;
      };
    };
    const spans = [...document.querySelectorAll<HTMLElement>("[data-start]")];
    const selection = getSelection()!;
    const range = document.createRange();
    range.setStart(spans[0]!.firstChild!, 1);
    range.setEnd(spans[4]!.firstChild!, spans[4]!.textContent!.length - 1);
    selection.removeAllRanges();
    selection.addRange(range);
    document.dispatchEvent(new Event("selectionchange"));
    await new Promise((resolve) => setTimeout(resolve, 0));
    const viewerSelection = state.__selectionViewer.getSelection();
    return {
      dom: selection.toString(),
      viewer: viewerSelection?.text,
      copied: await state.__selectionViewer.copySelection(),
      startOffset: viewerSelection?.startOffset,
      endOffset: viewerSelection?.endOffset,
    };
  });
  const expected = logicalText.slice(1);
  expect(result.dom).toBe(expected);
  expect(result.viewer).toBe(expected);
  expect(result.copied).toBe(expected);
  expect(result.startOffset).toBe(1);
  expect(result.endOffset).toBe(logicalText.length);
});

test("pointer drag works forward/backward and double-click selects a word", async ({
  page,
}) => {
  const spans = page.locator("[data-start]");
  const first = await spans.nth(0).boundingBox();
  const last = await spans.nth(4).boundingBox();
  if (!first || !last) throw new Error("Selection spans are not measurable");
  const firstPoint = { x: first.x + 3, y: first.y + first.height / 2 };
  const lastPoint = {
    x: last.x + Math.max(3, last.width - 3),
    y: last.y + last.height / 2,
  };

  await dragUntilSelection(page, firstPoint, lastPoint);
  await expect
    .poll(() => page.evaluate(() => getSelection()?.toString() ?? ""))
    .toContain("Привет 日本語 العربية");

  await page.evaluate(() => getSelection()?.removeAllRanges());
  await dragUntilSelection(page, lastPoint, firstPoint);
  await expect
    .poll(() => page.evaluate(() => getSelection()?.toString() ?? ""))
    .toContain("Привет 日本語 العربية");

  const cyrillic = await spans.nth(1).boundingBox();
  if (!cyrillic) throw new Error("Cyrillic span is not measurable");
  await page.evaluate(() => getSelection()?.removeAllRanges());
  await page.mouse.dblclick(
    cyrillic.x + cyrillic.width / 2,
    cyrillic.y + cyrillic.height / 2,
  );
  await expect
    .poll(() => page.evaluate(() => getSelection()?.toString().trim() ?? ""))
    .toBe("Привет");
});

test("search highlight is inert and separate from selectable spans", async ({
  page,
}) => {
  await page.evaluate(async () => {
    await (
      window as Window & {
        __selectionViewer: { search(query: string): Promise<unknown> };
      }
    ).__selectionViewer.search("Привет");
  });
  await expect(
    page.locator('[data-zrimo-layer="highlight"] > div'),
  ).toHaveCount(1);
  const styles = await page.locator("[data-start]").evaluateAll((spans) =>
    spans.map((span) => ({
      background: (span as HTMLElement).style.background,
      pointerEvents: getComputedStyle(span).pointerEvents,
    })),
  );
  expect(styles.every((style) => style.background === "")).toBe(true);
  expect(styles.every((style) => style.pointerEvents === "all")).toBe(true);
});

async function drag(
  page: import("@playwright/test").Page,
  from: { x: number; y: number },
  to: { x: number; y: number },
): Promise<void> {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 12 });
  await page.mouse.up();
}

async function dragUntilSelection(
  page: import("@playwright/test").Page,
  from: { x: number; y: number },
  to: { x: number; y: number },
): Promise<void> {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    await page.evaluate(() => getSelection()?.removeAllRanges());
    await drag(page, from, to);
    if (await page.evaluate(() => (getSelection()?.toString().length ?? 0) > 0))
      return;
    await page.waitForTimeout(50);
  }
}
