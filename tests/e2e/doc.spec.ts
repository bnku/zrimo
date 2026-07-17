import { expect, test } from "@playwright/test";

test("Word 97 DOC preserves structured content through browser WASM", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient } = await import("/main.js");
    const response = await fetch("/corpus/word97-simple-table.doc");
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer();
    try {
      await viewer.load(new Uint8Array(await response.arrayBuffer()), {
        fileName: "word97-simple-table.doc",
      });
      const pages = await Promise.all(
        Array.from({ length: viewer.state.pageCount }, (_, pageIndex) =>
          viewer.getPageText(pageIndex),
        ),
      );
      const canvas = document.createElement("canvas");
      await viewer.renderPage(0, canvas, {
        zoom: 1,
        devicePixelRatio: window.devicePixelRatio,
      });
      const token = pages.join("\n").match(/[A-Za-z]{3,}/u)?.[0] ?? "";
      const search = token ? await viewer.search(token) : null;
      return {
        format: viewer.state.format,
        status: viewer.state.status,
        pageCount: viewer.state.pageCount,
        textLength: pages.join("\n").trim().length,
        searchMatches: search?.matches.length ?? 0,
        width: canvas.width,
        height: canvas.height,
      };
    } finally {
      await viewer.destroy();
      await client.destroy();
    }
  });

  expect(result.status).toBe("ready");
  expect(result.format).toBe("doc");
  expect(result.pageCount).toBeGreaterThan(0);
  expect(result.textLength).toBeGreaterThan(20);
  expect(result.searchMatches).toBeGreaterThan(0);
  expect(result.width).toBeGreaterThan(0);
  expect(result.height).toBeGreaterThan(0);
});
