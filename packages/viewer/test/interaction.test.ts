import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  LruMap,
  cellRangeToTsv,
  findNormalizedMatches,
  normalizeCellRange,
  snapGraphemeOffset,
  visibleRange,
} from "../src/index.js";

describe("virtualization primitives", () => {
  it("keeps the visible interval bounded for thousands of units", () => {
    const range = visibleRange(500_000, 800, 1_080, 10_000, 2);
    assert.equal(range.end - range.start <= 6, true);
    assert.equal(range.start > 0, true);
    assert.equal(range.end < 10_000, true);
  });

  it("clamps edge ranges and evicts least-recently-used cache entries", () => {
    assert.deepEqual(visibleRange(-100, 500, 100, 3, 2), {
      start: 0,
      end: 3,
    });
    const cache = new LruMap<string, number>(2);
    cache.set("a", 1);
    cache.set("b", 2);
    assert.equal(cache.get("a"), 1);
    assert.deepEqual(cache.set("c", 3), { key: "b", value: 2 });
    assert.equal(cache.size, 2);
  });
});

describe("Unicode literal search", () => {
  it("uses NFKC and case folding while retaining original offsets", () => {
    const matches = findNormalizedMatches("До ＡＢＣ после", "abc", 4);
    assert.equal(matches.length, 1);
    assert.equal(matches[0]?.pageIndex, 4);
    assert.equal(matches[0]?.text, "ＡＢＣ");
  });

  it("handles Cyrillic case and CJK/Indic substrings", () => {
    assert.equal(findNormalizedMatches("ПрИвЕт", "привет", 0).length, 1);
    assert.equal(findNormalizedMatches("日本語文書", "語文", 0).length, 1);
    assert.equal(
      findNormalizedMatches("यह हिन्दी दस्तावेज़ है", "हिन्दी", 0).length,
      1,
    );
  });

  it("preserves Arabic diacritics instead of silently stripping them", () => {
    assert.equal(findNormalizedMatches("سَلَام", "سلام", 0).length, 0);
    assert.equal(findNormalizedMatches("سَلَام", "سَلَام", 0).length, 1);
  });
});

describe("grapheme-aware selection boundaries", () => {
  it("keeps UTF-16 public offsets but expands native endpoints to clusters", () => {
    const text = "A👨‍👩‍👧‍👦क्षिB";
    const emojiStart = 1;
    const emojiEnd = emojiStart + "👨‍👩‍👧‍👦".length;
    assert.equal(snapGraphemeOffset(text, emojiStart + 2, "start"), emojiStart);
    assert.equal(snapGraphemeOffset(text, emojiStart + 2, "end"), emojiEnd);
    const indicStart = emojiEnd;
    const indicEnd = indicStart + "क्षि".length;
    assert.equal(snapGraphemeOffset(text, indicStart + 1, "start"), indicStart);
    assert.equal(snapGraphemeOffset(text, indicStart + 1, "end"), indicEnd);
    assert.equal(snapGraphemeOffset(text, indicEnd, "end"), indicEnd);
  });
});

describe("cell range copy", () => {
  it("normalizes reverse ranges and emits escaped TSV", () => {
    const range = normalizeCellRange({
      sheetIndex: 0,
      startRow: 2,
      startColumn: 2,
      endRow: 1,
      endColumn: 1,
    });
    assert.deepEqual(range, {
      sheetIndex: 0,
      startRow: 1,
      startColumn: 1,
      endRow: 2,
      endColumn: 2,
    });
    assert.equal(
      cellRangeToTsv(
        range,
        new Map([
          ["1:1", "a"],
          ["1:2", "b\tquoted"],
          ["2:1", "line\nbreak"],
          ["2:2", "d"],
        ]),
      ),
      'a\t"b\tquoted"\n"line\nbreak"\td',
    );
  });
});
