import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";

import type {
  AdapterOpenContext,
  DocumentFormat,
  ResourceLimits,
} from "../src/index.js";
import {
  OfficeDocumentAdapter,
  ViewerError,
  defaultResourceLimits,
  sanitizeOfficeHyperlink,
  ViewerClient,
} from "../src/index.js";

const previousOffscreenCanvas = globalThis.OffscreenCanvas;

before(() => {
  Object.defineProperty(globalThis, "OffscreenCanvas", {
    configurable: true,
    value: class FakeOffscreenCanvas {
      width: number;
      height: number;
      constructor(width: number, height: number) {
        this.width = width;
        this.height = height;
      }
    },
  });
});

after(() => {
  Object.defineProperty(globalThis, "OffscreenCanvas", {
    configurable: true,
    value: previousOffscreenCanvas,
  });
});

describe("OfficeDocumentAdapter", () => {
  it("normalizes DOCX pages, text maps, links, and macro policy", async () => {
    let destroyed = 0;
    let renderedWidth = 0;
    const adapter = new OfficeDocumentAdapter({
      engines: {
        docx: async () => ({
          pageCount: 2,
          pageSize: () => ({ widthPt: 612, heightPt: 792 }),
          renderPage: async (_target, _index, options) => {
            renderedWidth = options.width;
          },
          collectPageRuns: async () => [
            {
              text: "Привет",
              x: 1,
              y: 2,
              w: 30,
              h: 12,
              fontSize: 12,
              font: '700 12px "Noto Sans"',
              letterSpacingPx: 0.5,
              hyperlink: { kind: "external", url: "https://example.com/a" },
            },
            {
              text: "blocked",
              x: 1,
              y: 15,
              w: 30,
              h: 12,
              fontSize: 12,
              font: 'italic 12px "Noto Sans"',
              transform: "rotate(90deg)",
              hyperlink: { kind: "external", url: "javascript:alert(1)" },
            },
            {
              text: "anchor",
              x: 1,
              y: 28,
              w: 30,
              h: 12,
              fontSize: 12,
              font: '12px "Noto Sans CJK"',
              eastAsianVert: true,
              hyperlink: { kind: "internal", ref: "chapter" },
            },
          ],
          getBookmarkPage: () => 1,
          destroy: () => {
            destroyed += 1;
          },
        }),
      },
    });
    const handle = await adapter.open(Uint8Array.of(1), context("docm"));
    const info = await adapter.getInfo(handle);
    assert.equal(info.pageCount, 2);
    assert.equal(info.unit, "page");
    assert.equal(info.warnings?.[0]?.details?.feature, "vba");

    await adapter.render(handle, new OffscreenCanvas(1, 1), {
      pageIndex: 0,
      zoom: 2,
      devicePixelRatio: 1,
    });
    assert.equal(renderedWidth, 1632);
    const runs = await adapter.getTextMap(handle, 0);
    assert.equal(runs[0]?.hyperlink?.kind, "external");
    assert.deepEqual(
      {
        font: runs[0]?.font,
        fontSize: runs[0]?.fontSize,
        fontFamily: runs[0]?.fontFamily,
        fontWeight: runs[0]?.fontWeight,
        letterSpacingPx: runs[0]?.letterSpacingPx,
        textLayer: runs[0]?.textLayer,
        coordinateWidth: runs[0]?.coordinateWidth,
        coordinateHeight: runs[0]?.coordinateHeight,
        logicalStart: runs[0]?.logicalStart,
        logicalEnd: runs[0]?.logicalEnd,
      },
      {
        font: '700 12px "Noto Sans"',
        fontSize: 12,
        fontFamily: "Noto Sans",
        fontWeight: 700,
        letterSpacingPx: 0.5,
        textLayer: "docx",
        coordinateWidth: 816,
        coordinateHeight: 1056,
        logicalStart: 0,
        logicalEnd: 6,
      },
    );
    assert.equal(runs[1]?.fontStyle, "italic");
    assert.equal(runs[1]?.transform, "rotate(90deg)");
    assert.equal(runs[2]?.eastAsianVert, true);
    assert.equal(runs[2]?.logicalStart, 13);
    assert.equal(runs[1]?.hyperlink, undefined);
    assert.deepEqual(runs[2]?.hyperlink, {
      kind: "internal",
      ref: "chapter",
      pageIndex: 1,
    });
    await adapter.close(handle);
    assert.equal(destroyed, 1);
  });

  it("normalizes presentations and resolves internal slide links", async () => {
    const adapter = new OfficeDocumentAdapter({
      engines: {
        pptx: async () => ({
          slideCount: 3,
          slideWidth: 9_144_000,
          slideHeight: 5_143_500,
          renderSlide: async () => {},
          collectSlideRuns: async () => [
            {
              text: "الشريحة",
              inShapeX: 4,
              inShapeY: 5,
              shapeX: 10,
              shapeY: 20,
              w: 60,
              h: 18,
              hyperlink: { kind: "internal", ref: "next" },
            },
          ],
          resolveInternalTarget: () => 2,
          destroy: () => {},
        }),
      },
    });
    const handle = await adapter.open(Uint8Array.of(1), context("ppsx"));
    const runs = await adapter.getTextMap(handle, 0);
    assert.equal((await adapter.getInfo(handle)).unit, "slide");
    assert.equal(runs[0]?.x, 14);
    assert.equal(runs[0]?.y, 25);
    assert.equal(runs[0]?.direction, "rtl");
    assert.equal(
      runs[0]?.hyperlink?.kind === "internal"
        ? runs[0].hyperlink.pageIndex
        : undefined,
      2,
    );
  });

  it("uses cached spreadsheet values, exposes sheet geometry, and never calculates formulas", async () => {
    let volatileFormulaAfterOpen: string | undefined = "not-rendered";
    let renderedOffsets: {
      x: number | undefined;
      y: number | undefined;
    } = { x: undefined, y: undefined };
    const worksheet = {
      name: "Данные",
      rows: [
        {
          index: 1,
          cells: [
            {
              row: 1,
              col: 1,
              value: { type: "number" as const, number: 42 },
              formula: "NOW()",
            },
            {
              row: 1,
              col: 2,
              value: { type: "empty" as const },
              formula: "1+1",
            },
          ],
        },
      ],
      mergeCells: [{ top: 1, left: 1, bottom: 1, right: 2 }],
      colWidths: { 2: 12 },
      rowHeights: { 1: 18 },
      colHidden: { 3: true },
      defaultColWidth: 9,
      defaultRowHeight: 15,
      freezeRows: 1,
      freezeCols: 2,
      hyperlinks: [
        { row: 1, col: 1, url: "https://example.com", location: null },
        { row: 1, col: 2, url: "data:text/html,bad", location: null },
      ],
    };
    const adapter = new OfficeDocumentAdapter({
      engines: {
        xlsx: async () => ({
          sheetNames: ["Данные"],
          sheetCount: 1,
          getWorksheet: async () => worksheet,
          renderViewport: async (_target, _index, _range, options) => {
            volatileFormulaAfterOpen = worksheet.rows[0]?.cells[0]?.formula;
            renderedOffsets = {
              x: options.scrollOffsetX,
              y: options.scrollOffsetY,
            };
            options.onTextRun?.({
              text: "42",
              x: 10,
              y: 20,
              width: 80,
              height: 20,
              row: 1,
              col: 1,
            });
          },
          destroy: () => {},
        }),
      },
    });
    const handle = await adapter.open(Uint8Array.of(1), context("xlsx"));
    const info = await adapter.getInfo(handle);
    assert.deepEqual(info.sheets?.[0], {
      name: "Данные",
      frozenRows: 1,
      frozenColumns: 2,
      mergedRanges: [{ startRow: 1, startColumn: 1, endRow: 1, endColumn: 2 }],
      maxRow: 1,
      maxColumn: 3,
      defaultColumnWidth: 72,
      defaultRowHeight: 20,
      columnWidths: { 2: 96, 3: 0 },
      rowHeights: { 1: 24 },
      rowHeaderWidth: 50,
      columnHeaderHeight: 22,
      rightToLeft: false,
    });
    assert.match(info.warnings?.[0]?.message ?? "", /no cached result/);
    assert.deepEqual(worksheet.rows[0]?.cells[1]?.value, {
      type: "text",
      text: "=1+1",
    });
    const runs = await adapter.getTextMap(handle, 0);
    assert.equal(volatileFormulaAfterOpen, "not-rendered");
    assert.equal(runs[0]?.row, 1);
    assert.equal(runs[0]?.column, 1);
    assert.equal(runs[0]?.hyperlink?.kind, "external");
    await adapter.render(
      handle,
      { width: 640, height: 480 } as OffscreenCanvas,
      {
        pageIndex: 0,
        zoom: 1,
        devicePixelRatio: 1,
        width: 640,
        height: 480,
        sheetRange: { row: 8, column: 5, rowCount: 20, columnCount: 10 },
        scrollOffsetX: 6.5,
        scrollOffsetY: 4.25,
      },
    );
    assert.equal(volatileFormulaAfterOpen, undefined);
    assert.deepEqual(renderedOffsets, { x: 6.5, y: 4.25 });
  });

  it("refuses the lossy legacy DOC projection before conversion", async () => {
    let converterCalled = false;
    const adapter = new OfficeDocumentAdapter({
      legacy: {
        convert: async () => {
          converterCalled = true;
          return new Uint8Array();
        },
      },
    });
    const original = Uint8Array.of(0xd0, 0xcf, 1, 2);
    const limits: ResourceLimits = {
      ...defaultResourceLimits,
      maxInputBytes: 1024,
    };
    await assert.rejects(
      adapter.open(original, context("doc", limits)),
      isCode("fidelity-unsupported"),
    );
    assert.equal(converterCalled, false);
  });

  it("maps encrypted backend errors and destroys a parsed handle on abort", async () => {
    const encrypted = new OfficeDocumentAdapter({
      engines: {
        docx: async () => {
          throw Object.assign(new Error("encrypted"), { code: "encrypted" });
        },
      },
    });
    await assert.rejects(
      encrypted.open(Uint8Array.of(1), context("docx")),
      isCode("encrypted-document"),
    );

    let destroyed = 0;
    const controller = new AbortController();
    const aborted = new OfficeDocumentAdapter({
      engines: {
        docx: async () => {
          controller.abort();
          return {
            pageCount: 1,
            pageSize: () => ({ widthPt: 1, heightPt: 1 }),
            renderPage: async () => {},
            collectPageRuns: async () => [],
            destroy: () => {
              destroyed += 1;
            },
          };
        },
      },
    });
    await assert.rejects(
      aborted.open(
        Uint8Array.of(1),
        context("docx", undefined, controller.signal),
      ),
      isCode("aborted"),
    );
    assert.equal(destroyed, 1);
  });
});

describe("Office hyperlink policy", () => {
  it("allows only host-safe external schemes", () => {
    assert.equal(
      sanitizeOfficeHyperlink({ kind: "external", url: "HTTPS://example.com" })
        ?.kind,
      "external",
    );
    for (const url of [
      "javascript:alert(1)",
      "data:text/html,bad",
      "file:///etc/passwd",
      "/relative/path",
    ])
      assert.equal(
        sanitizeOfficeHyperlink({ kind: "external", url }),
        undefined,
      );
  });
});

describe("default Office registration", () => {
  it("routes every supported Office extension without manual registration", () => {
    const registry = ViewerClient.create().registry;
    for (const format of [
      "docx",
      "docm",
      "xlsx",
      "xlsm",
      "pptx",
      "pptm",
      "ppsx",
      "doc",
      "xls",
      "ppt",
    ] as const)
      assert.equal(registry.resolve(format).id, "office");
  });
});

function context(
  format: DocumentFormat,
  limits: ResourceLimits = defaultResourceLimits,
  signal: AbortSignal = new AbortController().signal,
): AdapterOpenContext {
  return {
    format,
    signal,
    limits,
    reportProgress: () => {},
    reportWarning: () => {},
  };
}

function isCode(code: string): (error: unknown) => boolean {
  return (error) => error instanceof ViewerError && error.code === code;
}
