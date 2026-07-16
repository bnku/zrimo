import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { createViewer, formatFromFileName, sniffFormat } from "../src/index.js";

describe("SSR-safe public API", () => {
  it("constructs without window or document", () => {
    const viewer = createViewer({ locale: "ru" });
    assert.equal(viewer.state.status, "idle");
  });

  it("emits state changes and clamps zoom", () => {
    const viewer = createViewer();
    let calls = 0;
    viewer.on("statechange", () => {
      calls += 1;
    });
    viewer.setZoom(100);
    assert.equal(viewer.state.zoom, 8);
    assert.equal(calls, 1);
  });
});

describe("format detection", () => {
  it("normalizes aliases and URL suffixes", () => {
    assert.equal(formatFromFileName("REPORT.DOCX?download=1"), "docx");
    assert.equal(formatFromFileName("scan.tif#page=1"), "tiff");
  });

  it("detects unambiguous magic bytes", () => {
    assert.equal(sniffFormat(new TextEncoder().encode("%PDF-1.7")), "pdf");
    assert.equal(sniffFormat(Uint8Array.of(0xff, 0xd8, 0xff)), "jpeg");
  });
});
