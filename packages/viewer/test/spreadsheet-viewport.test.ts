import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { AxisGeometry } from "../src/spreadsheet-viewport.js";

describe("spreadsheet viewport geometry", () => {
  it("indexes a large sparse axis without scanning every band", () => {
    const axis = new AxisGeometry(1_000_000, 20, {
      2: 40,
      3: 0,
      500_000: 100,
      999_999: 5,
    });

    assert.equal(axis.offsetOf(1), 0);
    assert.equal(axis.offsetOf(2), 20);
    assert.equal(axis.offsetOf(3), 60);
    assert.equal(axis.offsetOf(4), 60);
    assert.equal(axis.indexAt(60), 4);
    assert.equal(axis.sizeOf(500_000), 100);
    assert.equal(axis.indexAt(axis.offsetOf(750_000) + 3), 750_000);
    assert.equal(axis.totalSize, 20_000_065);
  });

  it("clamps offsets and supports an empty logical axis", () => {
    const empty = new AxisGeometry(0, 20);
    assert.equal(empty.totalSize, 0);
    assert.equal(empty.indexAt(100), 1);

    const axis = new AxisGeometry(3, 10);
    assert.equal(axis.indexAt(-10), 1);
    assert.equal(axis.indexAt(10_000), 3);
    assert.equal(axis.offsetOf(4), 30);
  });
});
