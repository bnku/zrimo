import type { CellRange, SearchMatch } from "./contracts.js";

export interface VirtualRange {
  readonly start: number;
  readonly end: number;
}

export function visibleRange(
  scrollOffset: number,
  viewportExtent: number,
  itemExtent: number,
  itemCount: number,
  overscan = 1,
): VirtualRange {
  if (itemCount <= 0 || itemExtent <= 0) return { start: 0, end: 0 };
  const first = Math.floor(Math.max(0, scrollOffset) / itemExtent);
  const last = Math.ceil(
    (Math.max(0, scrollOffset) + Math.max(0, viewportExtent)) / itemExtent,
  );
  return {
    start: Math.max(0, first - Math.max(0, Math.trunc(overscan))),
    end: Math.min(itemCount, last + Math.max(0, Math.trunc(overscan))),
  };
}

export function normalizeCellRange(range: CellRange): CellRange {
  return {
    sheetIndex: Math.max(0, Math.trunc(range.sheetIndex)),
    startRow: Math.max(1, Math.min(range.startRow, range.endRow)),
    startColumn: Math.max(1, Math.min(range.startColumn, range.endColumn)),
    endRow: Math.max(1, Math.max(range.startRow, range.endRow)),
    endColumn: Math.max(1, Math.max(range.startColumn, range.endColumn)),
  };
}

export function findNormalizedMatches(
  text: string,
  query: string,
  pageIndex: number,
  caseSensitive = false,
): readonly SearchMatch[] {
  const haystack = normalizeWithMap(text, caseSensitive);
  const needle = normalizeSearchText(query, caseSensitive);
  if (!needle) return [];
  const matches: SearchMatch[] = [];
  let offset = 0;
  while (offset <= haystack.text.length - needle.length) {
    const found = haystack.text.indexOf(needle, offset);
    if (found < 0) break;
    const last = found + needle.length - 1;
    const start = haystack.starts[found] ?? 0;
    const end = haystack.ends[last] ?? start;
    matches.push({
      pageIndex,
      start,
      end,
      text: text.slice(start, end),
    });
    offset = found + Math.max(1, needle.length);
  }
  return matches;
}

export function normalizeSearchText(
  text: string,
  caseSensitive = false,
): string {
  const normalized = text.normalize("NFKC");
  return caseSensitive ? normalized : unicodeCaseFold(normalized);
}

/** Clamp a UTF-16 DOM offset to a grapheme boundary for native selection UI. */
export function snapGraphemeOffset(
  text: string,
  offset: number,
  edge: "start" | "end",
): number {
  const safeOffset = Math.max(0, Math.min(text.length, Math.trunc(offset)));
  for (const segment of graphemeSegments(text)) {
    if (safeOffset === segment.start || safeOffset === segment.end)
      return safeOffset;
    if (safeOffset > segment.start && safeOffset < segment.end)
      return edge === "start" ? segment.start : segment.end;
  }
  return safeOffset;
}

export function cellRangeToTsv(
  range: CellRange,
  cells: ReadonlyMap<string, string>,
): string {
  const normalized = normalizeCellRange(range);
  const rows: string[] = [];
  for (let row = normalized.startRow; row <= normalized.endRow; row += 1) {
    const values: string[] = [];
    for (
      let column = normalized.startColumn;
      column <= normalized.endColumn;
      column += 1
    )
      values.push(escapeTsv(cells.get(`${row}:${column}`) ?? ""));
    rows.push(values.join("\t"));
  }
  return rows.join("\n");
}

export class LruMap<K, V> {
  readonly #values = new Map<K, V>();
  readonly #capacity: number;

  constructor(capacity: number) {
    this.#capacity = Math.max(1, Math.trunc(capacity));
  }

  get size(): number {
    return this.#values.size;
  }

  get(key: K): V | undefined {
    const value = this.#values.get(key);
    if (value === undefined) return undefined;
    this.#values.delete(key);
    this.#values.set(key, value);
    return value;
  }

  set(key: K, value: V): { key: K; value: V } | undefined {
    if (this.#values.has(key)) this.#values.delete(key);
    this.#values.set(key, value);
    if (this.#values.size <= this.#capacity) return undefined;
    const oldest = this.#values.entries().next().value as [K, V];
    this.#values.delete(oldest[0]);
    return { key: oldest[0], value: oldest[1] };
  }

  clear(): void {
    this.#values.clear();
  }
}

function normalizeWithMap(
  text: string,
  caseSensitive: boolean,
): { text: string; starts: number[]; ends: number[] } {
  const output: string[] = [];
  const starts: number[] = [];
  const ends: number[] = [];
  const segments = graphemeSegments(text);
  for (const segment of segments) {
    const normalized = normalizeSearchText(segment.value, caseSensitive);
    output.push(normalized);
    for (let index = 0; index < normalized.length; index += 1) {
      starts.push(segment.start);
      ends.push(segment.end);
    }
  }
  return { text: output.join(""), starts, ends };
}

function graphemeSegments(
  text: string,
): readonly { value: string; start: number; end: number }[] {
  if (typeof Intl.Segmenter === "function") {
    const segmenter = new Intl.Segmenter(undefined, {
      granularity: "grapheme",
    });
    return [...segmenter.segment(text)].map((segment) => ({
      value: segment.segment,
      start: segment.index,
      end: segment.index + segment.segment.length,
    }));
  }
  const result: { value: string; start: number; end: number }[] = [];
  let offset = 0;
  for (const value of text) {
    result.push({ value, start: offset, end: offset + value.length });
    offset += value.length;
  }
  return result;
}

function unicodeCaseFold(text: string): string {
  return text
    .toLocaleLowerCase("und")
    .replaceAll("ß", "ss")
    .replaceAll("ς", "σ");
}

function escapeTsv(value: string): string {
  return /[\t\n\r"]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
}
