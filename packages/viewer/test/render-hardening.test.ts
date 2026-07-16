import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { DocumentAdapter, DocumentInfo, TextRun } from "../src/index.js";
import { RenderScheduler, ViewerClient, ViewerError } from "../src/index.js";

describe("priority render scheduler", () => {
  it("bounds concurrency and starts queued work by priority", async () => {
    const scheduler = new RenderScheduler(1);
    const controller = new AbortController();
    const order: string[] = [];
    let release!: () => void;
    const first = scheduler.run("background", controller.signal, async () => {
      order.push("active-background");
      await new Promise<void>((resolve) => {
        release = resolve;
      });
    });
    const last = scheduler.run("background", controller.signal, async () => {
      order.push("queued-background");
    });
    const adjacent = scheduler.run("adjacent", controller.signal, async () => {
      order.push("adjacent");
    });
    const visible = scheduler.run("visible", controller.signal, async () => {
      order.push("visible");
    });
    assert.equal(scheduler.active, 1);
    assert.equal(scheduler.queued, 3);
    release();
    await Promise.all([first, last, adjacent, visible]);
    assert.deepEqual(order, [
      "active-background",
      "visible",
      "adjacent",
      "queued-background",
    ]);
    assert.equal(scheduler.active, 0);
  });

  it("removes an aborted queued render without starting it", async () => {
    const scheduler = new RenderScheduler(1);
    const activeController = new AbortController();
    let release!: () => void;
    const active = scheduler.run(
      "visible",
      activeController.signal,
      () =>
        new Promise<void>((resolve) => {
          release = resolve;
        }),
    );
    const queuedController = new AbortController();
    let started = false;
    const queued = scheduler.run(
      "visible",
      queuedController.signal,
      async () => {
        started = true;
      },
    );
    queuedController.abort();
    await assert.rejects(queued, isCode("aborted"));
    assert.equal(started, false);
    release();
    await active;
  });
});

describe("runtime allocation budgets", () => {
  it("rejects excessive page counts before publishing ready", async () => {
    const viewer = ViewerClient.create({
      adapters: [adapter({ format: "pdf", unit: "page", pageCount: 3 })],
      limits: { maxDocumentUnits: 2 },
    }).createViewer();
    await assert.rejects(
      viewer.load(pdfBytes(), { fileName: "too-many.pdf" }),
      isCode("resource-limit"),
    );
    assert.equal(viewer.state.status, "error");
  });

  it("rejects text-map and render targets beyond configured memory", async () => {
    const runs: readonly TextRun[] = [
      { text: "x".repeat(256), x: 0, y: 0, width: 100, height: 20 },
    ];
    const viewer = ViewerClient.create({
      adapters: [adapter({ format: "pdf", unit: "page", pageCount: 1 }, runs)],
      limits: { maxTextMapBytes: 100, maxDecodedPixels: 1_000 },
    }).createViewer();
    await viewer.load(pdfBytes());
    await assert.rejects(viewer.getPageText(0), isCode("resource-limit"));
    await assert.rejects(
      viewer.renderPage(0, {} as OffscreenCanvas, {
        width: 100,
        height: 100,
      }),
      isCode("resource-limit"),
    );
  });
});

function adapter(
  info: DocumentInfo,
  runs: readonly TextRun[] = [],
): DocumentAdapter {
  return {
    id: "hardening-test",
    formats: ["pdf"],
    open: async () => ({}),
    getInfo: async () => info,
    render: async () => {},
    getTextMap: async () => runs,
    close: () => {},
  };
}

function pdfBytes(): Uint8Array {
  return new TextEncoder().encode("%PDF-1.7\nhardening");
}

function isCode(code: string): (error: unknown) => boolean {
  return (error) => error instanceof ViewerError && error.code === code;
}
