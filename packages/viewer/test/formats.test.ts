import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";

import type {
  AdapterOpenContext,
  DocumentFormat,
  PdfBackend,
  ResourceLimits,
  TiffBackend,
} from "../src/index.js";
import {
  CsvDocumentAdapter,
  ImageDocumentAdapter,
  PdfDocumentAdapter,
  SvgDocumentAdapter,
  ViewerClient,
  ViewerError,
  defaultResourceLimits,
  parseDelimitedBytes,
  parseDelimitedText,
  sanitizeSvg,
} from "../src/index.js";

const originalCreateImageBitmap = globalThis.createImageBitmap;
let closedBitmaps = 0;

before(() => {
  Object.defineProperty(globalThis, "createImageBitmap", {
    configurable: true,
    value: async () => ({
      width: 2,
      height: 3,
      close: () => {
        closedBitmaps += 1;
      },
    }),
  });
});

after(() => {
  Object.defineProperty(globalThis, "createImageBitmap", {
    configurable: true,
    value: originalCreateImageBitmap,
  });
});

describe("CSV and TSV", () => {
  it("preserves quoted newlines, empty strings, and multilingual text", () => {
    const rows = parseDelimitedText(
      'name,note,empty\n"Иван","行1\n行2",\n"علي","नमस्ते",x',
      ",",
    );
    assert.deepEqual(rows, [
      ["name", "note", "empty"],
      ["Иван", "行1\n行2", ""],
      ["علي", "नमस्ते", "x"],
    ]);
  });

  it("detects delimiters and UTF-16 BOM without changing cell strings", () => {
    const text = "a;b\r\n1;2";
    const utf16 = new Uint8Array(2 + text.length * 2);
    utf16.set([0xff, 0xfe]);
    for (let index = 0; index < text.length; index += 1) {
      utf16[2 + index * 2] = text.charCodeAt(index);
      utf16[3 + index * 2] = text.charCodeAt(index) >> 8;
    }
    const parsed = parseDelimitedBytes(utf16, "csv");
    assert.equal(parsed.encoding, "utf-16le");
    assert.equal(parsed.delimiter, ";");
    assert.deepEqual(parsed.rows[1], ["1", "2"]);
  });

  it("enforces a bounded cell count", () => {
    assert.throws(
      () => parseDelimitedText("a,b\nc,d", ",", 3),
      isCode("resource-limit"),
    );
  });

  it("exposes one sheet and positioned cell text", async () => {
    const adapter = new CsvDocumentAdapter({
      parse: async (data, format) => parseDelimitedBytes(data, format),
    });
    const handle = await adapter.open(
      new TextEncoder().encode("ключ,значение\nمرحبا,नमस्ते"),
      context("csv", { fileName: "localization.csv" }),
    );
    const info = await adapter.getInfo(handle);
    assert.equal(info.unit, "sheet");
    assert.equal(info.sheetNames?.[0], "localization");
    assert.equal(info.sheets?.[0]?.maxRow, 2);
    const runs = await adapter.getTextMap(handle, 0);
    assert.equal(runs.length, 4);
    assert.equal(runs[2]?.direction, "rtl");
    assert.equal(runs[3]?.text, "नमस्ते");
  });
});

describe("SVG security", () => {
  it("removes scripts, event handlers, foreign content, and external references", () => {
    const sanitized = sanitizeSvg(`
      <svg xmlns="http://www.w3.org/2000/svg" onload="steal()">
        <script>alert(1)</script>
        <foreignObject><iframe src="https://bad.invalid"/></foreignObject>
        <image href="data:image/png;base64,AAAA" />
        <a href="javascript:alert(1)"><rect width="10" height="10" /></a>
        <use href="#safe" />
      </svg>`);
    assert.doesNotMatch(
      sanitized,
      /script|foreignObject|iframe|onload|data:image|javascript:/i,
    );
    assert.match(sanitized, /href="#safe"/);
  });

  it("returns a warning when sanitization changes the source", async () => {
    const adapter = new SvgDocumentAdapter();
    const handle = await adapter.open(
      new TextEncoder().encode(
        '<svg xmlns="http://www.w3.org/2000/svg"><script/></svg>',
      ),
      context("svg"),
    );
    assert.equal(
      (await adapter.getInfo(handle)).warnings?.[0]?.code,
      "external-resource-blocked",
    );
    assert.deepEqual(await adapter.getTextMap(), []);
  });
});

describe("PDF adapter", () => {
  it("renders through a bounded cache and exposes Unicode glyph coordinates", async () => {
    let renders = 0;
    let closes = 0;
    const backend: PdfBackend = {
      pageCount: 1,
      renderPagePng: async () => {
        renders += 1;
        return png(2, 3);
      },
      pageTextJson: async () =>
        JSON.stringify({
          page_width: 612,
          page_height: 792,
          chars: [
            { char: "A", bbox: { x: 1, y: 2, width: 3, height: 4 } },
            { char: "ش", bbox: { x: 4, y: 2, width: 3, height: 4 } },
          ],
        }),
      close: () => {
        closes += 1;
      },
    };
    const adapter = new PdfDocumentAdapter({ open: async () => backend });
    const handle = await adapter.open(
      new TextEncoder().encode("%PDF-1.7"),
      context("pdf"),
    );
    const canvas = fakeCanvas();
    await adapter.render(handle, canvas, {
      pageIndex: 0,
      zoom: 1,
      devicePixelRatio: 1,
    });
    await adapter.render(handle, canvas, {
      pageIndex: 0,
      zoom: 1,
      devicePixelRatio: 1,
    });
    assert.equal(renders, 1);
    assert.equal(closedBitmaps >= 2, true);
    const runs = await adapter.getTextMap(handle, 0);
    assert.deepEqual(runs[0], {
      text: "A",
      x: 1,
      y: 2,
      width: 3,
      height: 4,
      direction: "ltr",
    });
    assert.equal(runs[1]?.direction, "rtl");
    await adapter.close(handle);
    assert.equal(closes, 1);
  });

  it("rejects encrypted PDFs before starting a backend", async () => {
    let opened = false;
    const adapter = new PdfDocumentAdapter({
      open: async () => {
        opened = true;
        throw new Error("unexpected");
      },
    });
    await assert.rejects(
      adapter.open(
        new TextEncoder().encode("%PDF-1.7\n/Encrypt 1 0 R"),
        context("pdf"),
      ),
      isCode("encrypted-document"),
    );
    assert.equal(opened, false);
  });
});

describe("image adapter", () => {
  it("uses native browser decoding for raster images", async () => {
    const adapter = new ImageDocumentAdapter();
    const handle = await adapter.open(png(2, 3), context("png"));
    assert.equal((await adapter.getInfo(handle)).pageCount, 1);
    await adapter.render(handle, fakeCanvas(), {
      pageIndex: 0,
      zoom: 2,
      devicePixelRatio: 1,
    });
    assert.deepEqual(await adapter.getTextMap(), []);
  });

  it("exposes multi-page TIFF and releases its worker backend", async () => {
    let closed = 0;
    const backend: TiffBackend = {
      pages: [
        { width: 2, height: 3 },
        { width: 4, height: 5 },
      ],
      renderPagePng: async () => png(4, 5),
      close: () => {
        closed += 1;
      },
    };
    const adapter = new ImageDocumentAdapter({ openTiff: async () => backend });
    const handle = await adapter.open(
      Uint8Array.of(0x49, 0x49, 0x2a, 0),
      context("tiff"),
    );
    assert.equal((await adapter.getInfo(handle)).pageCount, 2);
    await adapter.render(handle, fakeCanvas(), {
      pageIndex: 1,
      zoom: 1,
      devicePixelRatio: 1,
    });
    await adapter.close(handle);
    assert.equal(closed, 1);
  });
});

describe("default non-Office registration", () => {
  it("routes the complete agreed format matrix", () => {
    const registry = ViewerClient.create().registry;
    const expected: Readonly<Record<DocumentFormat, string>> = {
      docx: "office",
      docm: "office",
      xlsx: "office",
      xlsm: "office",
      pptx: "office",
      pptm: "office",
      ppsx: "office",
      doc: "office",
      xls: "office",
      ppt: "office",
      pdf: "pdf",
      csv: "delimited-data",
      tsv: "delimited-data",
      png: "image",
      jpeg: "image",
      gif: "image",
      webp: "image",
      svg: "svg",
      bmp: "image",
      tiff: "image",
    };
    for (const [format, adapterId] of Object.entries(expected))
      assert.equal(registry.resolve(format as DocumentFormat).id, adapterId);
  });
});

function context(
  format: DocumentFormat,
  extra: { fileName?: string; limits?: ResourceLimits } = {},
): AdapterOpenContext {
  return {
    format,
    signal: new AbortController().signal,
    limits: extra.limits ?? defaultResourceLimits,
    reportProgress: () => {},
    reportWarning: () => {},
    ...(extra.fileName ? { fileName: extra.fileName } : {}),
  };
}

function png(width: number, height: number): Uint8Array {
  const data = new Uint8Array(32);
  data.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const view = new DataView(data.buffer);
  view.setUint32(16, width);
  view.setUint32(20, height);
  return data;
}

function fakeCanvas(): HTMLCanvasElement | OffscreenCanvas {
  const context = {
    setTransform: () => {},
    clearRect: () => {},
    drawImage: () => {},
  };
  return {
    width: 1,
    height: 1,
    getContext: () => context,
  } as unknown as OffscreenCanvas;
}

function isCode(code: string): (error: unknown) => boolean {
  return (error) => error instanceof ViewerError && error.code === code;
}
