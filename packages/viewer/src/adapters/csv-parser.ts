import { ViewerError } from "../errors.js";

export interface ParsedDelimitedText {
  readonly delimiter: string;
  readonly encoding: string;
  readonly rows: readonly (readonly string[])[];
}

export function parseDelimitedBytes(
  data: Uint8Array,
  format: "csv" | "tsv",
  maxCells = 1_000_000,
): ParsedDelimitedText {
  const decoded = decodeDelimitedBytes(data);
  const delimiter =
    format === "tsv" ? "\t" : detectDelimiter(decoded.text, [",", "\t", ";"]);
  return {
    delimiter,
    encoding: decoded.encoding,
    rows: parseDelimitedText(decoded.text, delimiter, maxCells),
  };
}

export function parseDelimitedText(
  text: string,
  delimiter: string,
  maxCells = 1_000_000,
): readonly (readonly string[])[] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  let cells = 0;
  const pushField = (): void => {
    cells += 1;
    if (cells > maxCells)
      throw new ViewerError(
        "resource-limit",
        "Delimited data exceeds cell limit",
        {
          details: { maxCells },
        },
      );
    row.push(field);
    field = "";
  };
  const pushRow = (): void => {
    pushField();
    rows.push(row);
    row = [];
  };

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index]!;
    if (quoted) {
      if (character === '"') {
        if (text[index + 1] === '"') {
          field += '"';
          index += 1;
        } else quoted = false;
      } else field += character;
      continue;
    }
    if (character === '"' && field.length === 0) quoted = true;
    else if (character === delimiter) pushField();
    else if (character === "\n") pushRow();
    else if (character !== "\r") field += character;
  }
  if (quoted)
    throw new ViewerError(
      "invalid-file",
      "Unterminated quoted field in delimited data",
    );
  if (field.length > 0 || row.length > 0) pushRow();
  return rows;
}

function decodeDelimitedBytes(data: Uint8Array): {
  text: string;
  encoding: string;
} {
  if (data[0] === 0xef && data[1] === 0xbb && data[2] === 0xbf)
    return {
      text: new TextDecoder("utf-8", { fatal: true }).decode(data.subarray(3)),
      encoding: "utf-8",
    };
  if (data[0] === 0xff && data[1] === 0xfe)
    return {
      text: new TextDecoder("utf-16le", { fatal: true }).decode(
        data.subarray(2),
      ),
      encoding: "utf-16le",
    };
  if (data[0] === 0xfe && data[1] === 0xff) {
    const swapped = data.subarray(2).slice();
    for (let index = 0; index + 1 < swapped.length; index += 2)
      [swapped[index], swapped[index + 1]] = [
        swapped[index + 1]!,
        swapped[index]!,
      ];
    return {
      text: new TextDecoder("utf-16le", { fatal: true }).decode(swapped),
      encoding: "utf-16be",
    };
  }
  try {
    return {
      text: new TextDecoder("utf-8", { fatal: true }).decode(data),
      encoding: "utf-8",
    };
  } catch {
    return {
      text: new TextDecoder("windows-1252").decode(data),
      encoding: "windows-1252",
    };
  }
}

function detectDelimiter(text: string, candidates: readonly string[]): string {
  const sample = text.slice(0, 64 * 1024);
  let best = ",";
  let bestScore = -1;
  for (const candidate of candidates) {
    let quoted = false;
    let count = 0;
    let lines = 0;
    for (let index = 0; index < sample.length && lines < 20; index += 1) {
      const character = sample[index]!;
      if (character === '"') {
        if (quoted && sample[index + 1] === '"') index += 1;
        else quoted = !quoted;
      } else if (!quoted && character === candidate) count += 1;
      else if (!quoted && character === "\n") lines += 1;
    }
    if (count > bestScore) {
      best = candidate;
      bestScore = count;
    }
  }
  return best;
}
