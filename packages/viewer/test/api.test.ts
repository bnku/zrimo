import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type {
  AdapterOpenContext,
  DocumentAdapter,
  DocumentInfo,
  RenderViewport,
  TextRun,
} from "../src/index.js";
import { ViewerClient, ViewerError } from "../src/index.js";

const pdfBytes = new TextEncoder().encode("%PDF-1.7\nviewer-api");

describe("public headless API", () => {
  it("renders pages/thumbnails, navigates, pans, fits, and returns immutable info", async () => {
    const renders: RenderViewport[] = [];
    const adapter = textAdapter(renders);
    const viewer = ViewerClient.create({ adapters: [adapter] }).createViewer();
    await viewer.load(pdfBytes, { fileName: "source.pdf" });

    const info = viewer.getDocumentInfo();
    assert.equal(info.pageCount, 3);
    assert.equal(info.capabilities?.textSelection, true);
    assert.equal(Object.isFrozen(info), true);
    assert.equal(Object.isFrozen(viewer.state), true);

    viewer.next();
    assert.equal(viewer.state.pageIndex, 1);
    viewer.previous();
    assert.equal(viewer.state.pageIndex, 0);
    viewer.goToPage(99);
    assert.equal(viewer.state.pageIndex, 2);
    viewer.panBy(12, 34);
    assert.deepEqual([viewer.state.panX, viewer.state.panY], [12, 34]);
    viewer.setZoom(2);
    viewer.fitWidth();
    assert.equal(viewer.state.fit, "width");
    viewer.fitPage();
    assert.equal(viewer.state.fit, "page");

    const canvas = {} as OffscreenCanvas;
    await viewer.renderPage(1, canvas, { zoom: 1.5, devicePixelRatio: 2 });
    await viewer.renderThumbnail(2, canvas, { maxWidth: 100, maxHeight: 100 });
    assert.equal(renders[0]?.pageIndex, 1);
    assert.equal(renders[0]?.zoom, 1.5);
    assert.equal(renders[0]?.devicePixelRatio, 2);
    assert.equal((renders[1]?.zoom ?? 2) < 1, true);

    assert.equal(await viewer.getPageText(0), "Привет мир");
    assert.deepEqual(viewer.getOriginalBytes(), pdfBytes);
    const download = viewer.downloadOriginal("kept.pdf");
    assert.equal(download.size, pdfBytes.length);
  });

  it("searches logical Unicode, cycles matches, and selects across pages", async () => {
    const viewer = ViewerClient.create({
      adapters: [textAdapter()],
    }).createViewer();
    await viewer.load(pdfBytes);
    const searchEvents: (number | null)[] = [];
    viewer.on("searchchange", (result) =>
      searchEvents.push(result?.activeIndex ?? null),
    );

    const result = await viewer.search("ПРИВЕТ");
    assert.equal(result.matches.length, 2);
    assert.equal(result.matches[0]?.pageIndex, 0);
    assert.equal(viewer.searchNext()?.activeIndex, 1);
    assert.equal(viewer.state.pageIndex, 1);
    assert.equal(viewer.searchPrevious()?.activeIndex, 0);

    const selection = await viewer.selectText({
      startPageIndex: 0,
      startOffset: 7,
      endPageIndex: 1,
      endOffset: 6,
    });
    assert.equal(selection.text, "мир\nВторой");
    assert.equal(await viewer.copySelection(), "мир\nВторой");
    assert.equal(viewer.getSelection(), selection);
    viewer.clearSelection();
    assert.equal(viewer.getSelection(), null);
    viewer.clearSearch();
    assert.equal(searchEvents.at(-1), null);
  });

  it("cancels headless rendering on close and rejects stale completion", async () => {
    let started: (() => void) | undefined;
    const rendering = new Promise<void>((resolve) => {
      started = resolve;
    });
    const adapter = textAdapter();
    adapter.render = async (_handle, _target, _viewport, signal) => {
      started?.();
      await new Promise<void>((_resolve, reject) =>
        signal?.addEventListener(
          "abort",
          () => reject(new DOMException("aborted", "AbortError")),
          { once: true },
        ),
      );
    };
    const viewer = ViewerClient.create({ adapters: [adapter] }).createViewer();
    await viewer.load(pdfBytes);
    const pending = viewer.renderPage(0, {} as OffscreenCanvas);
    await rendering;
    await viewer.close();
    await assert.rejects(pending, isCode("aborted"));
  });
});

describe("spreadsheet interaction contract", () => {
  it("renders a sheet viewport, expands merged selection, and copies TSV", async () => {
    let rendered: RenderViewport | undefined;
    const info: DocumentInfo = {
      format: "xlsx",
      unit: "sheet",
      pageCount: 1,
      sheetNames: ["Data"],
      sheets: [
        {
          name: "Data",
          frozenRows: 1,
          frozenColumns: 0,
          maxRow: 2,
          maxColumn: 2,
          mergedRanges: [
            { startRow: 1, startColumn: 1, endRow: 1, endColumn: 2 },
          ],
        },
      ],
    };
    const adapter: DocumentAdapter = {
      id: "sheet-test",
      formats: ["xlsx"],
      open: async () => ({}),
      getInfo: async () => info,
      render: async (_handle, _target, viewport) => {
        rendered = viewport;
      },
      getTextMap: async () => [
        cell("A", 1, 1),
        cell("", 1, 2),
        cell("C", 2, 1),
        cell("D", 2, 2),
      ],
      close: () => {},
    };
    const viewer = ViewerClient.create({ adapters: [adapter] }).createViewer();
    await viewer.load(ooxmlBytes(), { fileName: "sheet.xlsx" });
    await viewer.renderSheetViewport(
      0,
      {} as OffscreenCanvas,
      { row: 5, column: 3, rowCount: 10, columnCount: 4 },
      { width: 640, height: 480 },
    );
    assert.deepEqual(rendered?.sheetRange, {
      row: 5,
      column: 3,
      rowCount: 10,
      columnCount: 4,
    });
    const selected = viewer.selectCells({
      sheetIndex: 0,
      startRow: 1,
      startColumn: 1,
      endRow: 1,
      endColumn: 1,
    });
    assert.equal(selected.endColumn, 2);
    assert.equal(await viewer.copySelection(), "A\t");
    viewer.setSheet(0);
    viewer.setSheet(1);
    assert.equal(viewer.state.pageIndex, 0);
  });
});

function textAdapter(renders: RenderViewport[] = []): DocumentAdapter {
  const info: DocumentInfo = { format: "pdf", unit: "page", pageCount: 3 };
  const text: readonly (readonly TextRun[])[] = [
    [run("Привет ", 0), run("мир", 50)],
    [run("Второй привет", 0)],
    [run("日本語 हिन्दी سَلَام", 0)],
  ];
  return {
    id: "text-test",
    formats: ["pdf"],
    open: async (_data, _context: AdapterOpenContext) => ({}),
    getInfo: async () => info,
    render: async (_handle, _target, viewport) => {
      renders.push(viewport);
    },
    getTextMap: async (_handle, pageIndex) => text[pageIndex] ?? [],
    close: () => {},
  };
}

function run(text: string, x: number): TextRun {
  return {
    text,
    x,
    y: 0,
    width: text.length * 8,
    height: 16,
    direction: "ltr",
  };
}

function cell(text: string, row: number, column: number): TextRun {
  return {
    text,
    x: column * 100,
    y: row * 24,
    width: 100,
    height: 24,
    row,
    column,
  };
}

function ooxmlBytes(): Uint8Array {
  const data = new Uint8Array(128);
  data.set([0x50, 0x4b, 3, 4]);
  data.set(new TextEncoder().encode("xl/workbook.xml"), 8);
  new DataView(data.buffer).setUint32(64, 0x02014b50, true);
  return data;
}

function isCode(code: string): (error: unknown) => boolean {
  return (error) => error instanceof ViewerError && error.code === code;
}
