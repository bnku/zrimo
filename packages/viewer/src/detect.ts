import type { DocumentFormat, ViewerWarning } from "./contracts.js";
import { ViewerError } from "./errors.js";

export interface FormatHints {
  readonly fileName?: string;
  readonly contentType?: string;
  readonly format?: DocumentFormat;
}

export interface FormatDetection {
  readonly format: DocumentFormat;
  readonly confidence: "magic" | "container" | "hint";
  readonly warnings: readonly ViewerWarning[];
}

const extensionFormats: Readonly<Record<string, DocumentFormat>> = {
  docx: "docx",
  docm: "docm",
  xlsx: "xlsx",
  xlsm: "xlsm",
  pptx: "pptx",
  pptm: "pptm",
  ppsx: "ppsx",
  doc: "doc",
  xls: "xls",
  ppt: "ppt",
  pdf: "pdf",
  csv: "csv",
  tsv: "tsv",
  png: "png",
  jpg: "jpeg",
  jpeg: "jpeg",
  gif: "gif",
  webp: "webp",
  svg: "svg",
  bmp: "bmp",
  tif: "tiff",
  tiff: "tiff",
};

const mimeFormats: Readonly<Record<string, DocumentFormat>> = {
  "application/pdf": "pdf",
  "application/msword": "doc",
  "application/vnd.ms-excel": "xls",
  "application/vnd.ms-powerpoint": "ppt",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document":
    "docx",
  "application/vnd.ms-word.document.macroenabled.12": "docm",
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": "xlsx",
  "application/vnd.ms-excel.sheet.macroenabled.12": "xlsm",
  "application/vnd.openxmlformats-officedocument.presentationml.presentation":
    "pptx",
  "application/vnd.ms-powerpoint.presentation.macroenabled.12": "pptm",
  "application/vnd.openxmlformats-officedocument.presentationml.slideshow":
    "ppsx",
  "text/csv": "csv",
  "text/tab-separated-values": "tsv",
  "image/png": "png",
  "image/jpeg": "jpeg",
  "image/gif": "gif",
  "image/webp": "webp",
  "image/svg+xml": "svg",
  "image/bmp": "bmp",
  "image/tiff": "tiff",
};

export function detectFormat(
  data: Uint8Array,
  hints: FormatHints = {},
): FormatDetection {
  if (isEncryptedOfficeContainer(data))
    throw new ViewerError(
      "encrypted-document",
      "Password-protected Office documents are not supported",
    );
  const hinted =
    hints.format ??
    formatFromMime(hints.contentType) ??
    formatFromFileName(hints.fileName ?? "");
  const detected = detectByBytes(data, hinted);
  if (!detected && hinted && ["csv", "tsv", "svg"].includes(hinted))
    return { format: hinted, confidence: "hint", warnings: [] };
  if (!detected)
    throw new ViewerError(
      "unsupported-format",
      "Unable to detect a supported document format",
      {
        details: { fileName: hints.fileName, contentType: hints.contentType },
      },
    );

  const format = preserveSubtype(detected, hinted);
  const warnings: ViewerWarning[] = [];
  if (hinted && !sameFamily(format, hinted))
    warnings.push({
      code: "format-hint-mismatch",
      message: `Content is ${format}, but the supplied hint was ${hinted}; content wins`,
      details: { detected: format, hinted },
    });
  return {
    format,
    confidence: detected === format ? "magic" : "container",
    warnings,
  };
}

function isEncryptedOfficeContainer(data: Uint8Array): boolean {
  return (
    starts(data, [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) &&
    includesUtf16(data, "EncryptionInfo") &&
    includesUtf16(data, "EncryptedPackage")
  );
}

export function formatFromFileName(
  fileName: string,
): DocumentFormat | undefined {
  const cleanName = fileName.split(/[?#]/, 1)[0] ?? fileName;
  const dot = cleanName.lastIndexOf(".");
  return dot < 0
    ? undefined
    : extensionFormats[cleanName.slice(dot + 1).toLowerCase()];
}

export function formatFromMime(
  contentType?: string,
): DocumentFormat | undefined {
  return contentType
    ? mimeFormats[contentType.split(";", 1)[0]!.trim().toLowerCase()]
    : undefined;
}

export function sniffFormat(data: Uint8Array): DocumentFormat | undefined {
  return detectByBytes(data);
}

function detectByBytes(
  data: Uint8Array,
  hint?: DocumentFormat,
): DocumentFormat | undefined {
  if (starts(data, [0x25, 0x50, 0x44, 0x46, 0x2d])) return "pdf";
  if (starts(data, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]))
    return "png";
  if (starts(data, [0xff, 0xd8, 0xff])) return "jpeg";
  if (ascii(data, 0, 4) === "GIF8") return "gif";
  if (ascii(data, 0, 4) === "RIFF" && ascii(data, 8, 12) === "WEBP")
    return "webp";
  if (ascii(data, 0, 2) === "BM") return "bmp";
  if (
    starts(data, [0x49, 0x49, 0x2a, 0x00]) ||
    starts(data, [0x4d, 0x4d, 0x00, 0x2a])
  )
    return "tiff";
  if (starts(data, [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]))
    return detectOle(data) ?? legacyHint(hint);
  if (starts(data, [0x50, 0x4b])) return detectOoxml(data) ?? ooxmlHint(hint);
  if (looksLikeSvg(data)) return "svg";
  return undefined;
}

function detectOoxml(data: Uint8Array): DocumentFormat | undefined {
  if (includesAscii(data, "word/")) return "docx";
  if (includesAscii(data, "xl/")) return "xlsx";
  if (includesAscii(data, "ppt/")) return "pptx";
  return undefined;
}

function detectOle(data: Uint8Array): DocumentFormat | undefined {
  if (includesUtf16(data, "WordDocument")) return "doc";
  if (includesUtf16(data, "Workbook") || includesUtf16(data, "Book"))
    return "xls";
  if (includesUtf16(data, "PowerPoint Document")) return "ppt";
  return undefined;
}

function preserveSubtype(
  detected: DocumentFormat,
  hinted?: DocumentFormat,
): DocumentFormat {
  return hinted && sameFamily(detected, hinted) ? hinted : detected;
}

function sameFamily(a: DocumentFormat, b: DocumentFormat): boolean {
  return family(a) === family(b);
}

function family(format: DocumentFormat): string {
  if (["doc", "docx", "docm"].includes(format)) return "word";
  if (["xls", "xlsx", "xlsm"].includes(format)) return "sheet";
  if (["ppt", "pptx", "pptm", "ppsx"].includes(format)) return "slides";
  return format;
}

function legacyHint(format?: DocumentFormat): DocumentFormat | undefined {
  return format && ["doc", "xls", "ppt"].includes(format) ? format : undefined;
}

function ooxmlHint(format?: DocumentFormat): DocumentFormat | undefined {
  return format &&
    ["docx", "docm", "xlsx", "xlsm", "pptx", "pptm", "ppsx"].includes(format)
    ? format
    : undefined;
}

function looksLikeSvg(data: Uint8Array): boolean {
  const prefix = new TextDecoder().decode(
    data.subarray(0, Math.min(data.byteLength, 4096)),
  );
  return /<(?:svg)(?:\s|>)/i.test(prefix.replace(/^\uFEFF/, ""));
}

function starts(data: Uint8Array, signature: readonly number[]): boolean {
  return signature.every((byte, index) => data[index] === byte);
}

function ascii(data: Uint8Array, start: number, end: number): string {
  return String.fromCharCode(...data.subarray(start, end));
}

function includesAscii(data: Uint8Array, text: string): boolean {
  return indexOf(data, new TextEncoder().encode(text)) >= 0;
}

function includesUtf16(data: Uint8Array, text: string): boolean {
  const bytes = new Uint8Array(text.length * 2);
  for (let index = 0; index < text.length; index += 1)
    bytes[index * 2] = text.charCodeAt(index);
  return indexOf(data, bytes) >= 0;
}

function indexOf(haystack: Uint8Array, needle: Uint8Array): number {
  outer: for (
    let offset = 0;
    offset <= haystack.length - needle.length;
    offset += 1
  ) {
    for (let index = 0; index < needle.length; index += 1)
      if (haystack[offset + index] !== needle[index]) continue outer;
    return offset;
  }
  return -1;
}
