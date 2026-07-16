import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type {
  AdapterOpenContext,
  DocumentAdapter,
  DocumentInfo,
} from "../src/index.js";
import { ViewerClient, ViewerError } from "../src/index.js";

const pdfBytes = new TextEncoder().encode("%PDF-1.7\nqualification");
const info: DocumentInfo = { format: "pdf", unit: "page", pageCount: 2 };

describe("ViewerClient and DocumentViewer lifecycle", () => {
  it("loads, retains the original, closes, and loads again", async () => {
    let closes = 0;
    const adapter = mockAdapter({ close: () => closes++ });
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer();

    await viewer.load(pdfBytes);
    assert.equal(viewer.state.status, "ready");
    assert.equal(viewer.state.pageCount, 2);
    assert.deepEqual(viewer.getOriginalBytes(), pdfBytes);
    await viewer.close();
    assert.equal(viewer.state.status, "idle");
    assert.equal(viewer.getOriginalBytes(), undefined);
    await viewer.load(pdfBytes);
    assert.equal(closes, 1);
    await client.destroy();
    assert.equal(closes, 2);
  });

  it("cancels an in-flight adapter operation with a stable error", async () => {
    const adapter = mockAdapter({
      open: async (_data, context) =>
        new Promise((_, reject) =>
          context.signal.addEventListener(
            "abort",
            () => reject(new DOMException("aborted", "AbortError")),
            { once: true },
          ),
        ),
    });
    const viewer = ViewerClient.create({ adapters: [adapter] }).createViewer();
    const controller = new AbortController();
    const opening = viewer.load(pdfBytes, { signal: controller.signal });
    controller.abort();
    await assert.rejects(opening, isCode("aborted"));
  });

  it("newer load cancels an older load and wins deterministically", async () => {
    let call = 0;
    let markStarted: (() => void) | undefined;
    const started = new Promise<void>((resolve) => {
      markStarted = resolve;
    });
    const adapter = mockAdapter({
      open: async (_data, context) => {
        call += 1;
        if (call === 2) return { generation: 2 };
        markStarted?.();
        return new Promise((_, reject) =>
          context.signal.addEventListener(
            "abort",
            () => reject(new DOMException("aborted", "AbortError")),
            {
              once: true,
            },
          ),
        );
      },
    });
    const viewer = ViewerClient.create({ adapters: [adapter] }).createViewer();
    const first = viewer.load(pdfBytes);
    await started;
    const second = viewer.load(pdfBytes);
    await assert.rejects(first, isCode("aborted"));
    await second;
    assert.equal(viewer.state.status, "ready");
  });

  it("destroy is idempotent and later operations fail", async () => {
    const client = ViewerClient.create({ adapters: [mockAdapter()] });
    const viewer = client.createViewer();
    await viewer.destroy();
    await viewer.destroy();
    assert.throws(() => viewer.setZoom(2), isCode("lifecycle-error"));
  });

  it("uses custom fetch and maps network failures", async () => {
    let calls = 0;
    const client = ViewerClient.create({
      adapters: [mockAdapter()],
      fetch: async () => {
        calls += 1;
        return new Response(pdfBytes, {
          headers: { "content-type": "application/pdf" },
        });
      },
    });
    await client.createViewer().load("https://documents.invalid/test.pdf");
    assert.equal(calls, 1);

    const failing = ViewerClient.create({
      adapters: [mockAdapter()],
      fetch: async () => {
        throw new TypeError("CORS");
      },
    });
    await assert.rejects(
      failing.createViewer().load("https://documents.invalid/test.pdf"),
      isCode("network-error"),
    );
  });

  it("rejects an oversized Blob before reading it", async () => {
    const client = ViewerClient.create({
      adapters: [mockAdapter()],
      limits: { maxInputBytes: 4 },
    });
    await assert.rejects(
      client.createViewer().load(new Blob([pdfBytes])),
      isCode("resource-limit"),
    );
  });
});

function mockAdapter(
  overrides: {
    open?: (data: Uint8Array, context: AdapterOpenContext) => Promise<unknown>;
    close?: (handle: unknown) => void;
  } = {},
): DocumentAdapter {
  return {
    id: "mock-pdf",
    formats: ["pdf"],
    open: overrides.open ?? (async () => ({})),
    getInfo: async () => info,
    render: async () => {},
    close: overrides.close ?? (() => {}),
  };
}

function isCode(code: string): (error: unknown) => boolean {
  return (error) => error instanceof ViewerError && error.code === code;
}
