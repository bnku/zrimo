import type { DocumentFormat, ResourceLimits } from "./contracts.js";
import { ViewerError } from "./errors.js";

export const defaultResourceLimits: Readonly<ResourceLimits> = Object.freeze({
  maxInputBytes: 100 * 1024 * 1024,
  maxExpandedOfficeBytes: 512 * 1024 * 1024,
  maxZipEntryBytes: 64 * 1024 * 1024,
  maxDecodedPixels: 100_000_000,
  maxSvgBytes: 16 * 1024 * 1024,
  maxCsvCells: 1_000_000,
  maxTextMapBytes: 64 * 1024 * 1024,
  maxDocumentUnits: 100_000,
  maxConcurrentRenders: 2,
  maxOperationMs: 30_000,
});

export function resolveLimits(
  base: Partial<ResourceLimits> = {},
  override: Partial<ResourceLimits> = {},
): ResourceLimits {
  const result = { ...defaultResourceLimits, ...base, ...override };
  for (const [name, value] of Object.entries(result)) {
    if (!Number.isSafeInteger(value) || value <= 0)
      throw new ViewerError(
        "resource-limit",
        `Invalid resource limit ${name}: ${value}`,
      );
  }
  return result;
}

export function enforceContainerLimits(
  data: Uint8Array,
  format: DocumentFormat,
  limits: ResourceLimits,
): void {
  if (data.byteLength > limits.maxInputBytes)
    throw limitError(
      "Input exceeds maxInputBytes",
      data.byteLength,
      limits.maxInputBytes,
    );
  if (isZipOffice(format)) enforceZipLimits(data, limits);
  if (format === "svg" && data.byteLength > limits.maxSvgBytes)
    throw limitError(
      "SVG exceeds maxSvgBytes",
      data.byteLength,
      limits.maxSvgBytes,
    );
  const dimensions = imageDimensions(data, format);
  if (dimensions) {
    const pixels = dimensions.width * dimensions.height;
    if (!Number.isSafeInteger(pixels) || pixels > limits.maxDecodedPixels)
      throw limitError(
        "Decoded image exceeds maxDecodedPixels",
        pixels,
        limits.maxDecodedPixels,
      );
  }
}

function enforceZipLimits(data: Uint8Array, limits: ResourceLimits): void {
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  let expanded = 0;
  let entries = 0;
  for (let offset = 0; offset + 46 <= data.byteLength; offset += 1) {
    if (view.getUint32(offset, true) !== 0x02014b50) continue;
    const uncompressed = view.getUint32(offset + 24, true);
    if (uncompressed === 0xffffffff)
      throw new ViewerError(
        "resource-limit",
        "ZIP64 entries require explicit production support",
      );
    if (uncompressed > limits.maxZipEntryBytes)
      throw limitError(
        "ZIP entry exceeds maxZipEntryBytes",
        uncompressed,
        limits.maxZipEntryBytes,
      );
    expanded += uncompressed;
    if (expanded > limits.maxExpandedOfficeBytes)
      throw limitError(
        "Office package exceeds maxExpandedOfficeBytes",
        expanded,
        limits.maxExpandedOfficeBytes,
      );
    const nameLength = view.getUint16(offset + 28, true);
    const extraLength = view.getUint16(offset + 30, true);
    const commentLength = view.getUint16(offset + 32, true);
    offset += 45 + nameLength + extraLength + commentLength;
    entries += 1;
  }
  if (entries === 0)
    throw new ViewerError(
      "invalid-file",
      "OOXML ZIP has no central-directory entries",
    );
}

function imageDimensions(
  data: Uint8Array,
  format: DocumentFormat,
): { width: number; height: number } | undefined {
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  if (format === "png" && data.byteLength >= 24)
    return { width: view.getUint32(16), height: view.getUint32(20) };
  if (format === "gif" && data.byteLength >= 10)
    return { width: view.getUint16(6, true), height: view.getUint16(8, true) };
  if (format === "bmp" && data.byteLength >= 26)
    return {
      width: Math.abs(view.getInt32(18, true)),
      height: Math.abs(view.getInt32(22, true)),
    };
  if (format === "jpeg") return jpegDimensions(data);
  if (format === "webp") return webpDimensions(data);
  if (format === "tiff") return tiffDimensions(data);
  return undefined;
}

function webpDimensions(
  data: Uint8Array,
): { width: number; height: number } | undefined {
  if (data.byteLength < 25) return undefined;
  const chunk = ascii(data, 12, 16);
  if (chunk === "VP8X" && data.byteLength >= 30)
    return {
      width: 1 + data[24]! + (data[25]! << 8) + (data[26]! << 16),
      height: 1 + data[27]! + (data[28]! << 8) + (data[29]! << 16),
    };
  if (chunk === "VP8 " && data.byteLength >= 30)
    return {
      width: (data[26]! | (data[27]! << 8)) & 0x3fff,
      height: (data[28]! | (data[29]! << 8)) & 0x3fff,
    };
  if (chunk === "VP8L" && data[20] === 0x2f) {
    const bits =
      data[21]! | (data[22]! << 8) | (data[23]! << 16) | (data[24]! << 24);
    return {
      width: (bits & 0x3fff) + 1,
      height: ((bits >>> 14) & 0x3fff) + 1,
    };
  }
  return undefined;
}

function tiffDimensions(
  data: Uint8Array,
): { width: number; height: number } | undefined {
  if (data.byteLength < 10) return undefined;
  const littleEndian = ascii(data, 0, 2) === "II";
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const ifdOffset = view.getUint32(4, littleEndian);
  if (ifdOffset + 2 > data.byteLength) return undefined;
  const count = view.getUint16(ifdOffset, littleEndian);
  let width: number | undefined;
  let height: number | undefined;
  for (let index = 0; index < count; index += 1) {
    const offset = ifdOffset + 2 + index * 12;
    if (offset + 12 > data.byteLength) return undefined;
    const tag = view.getUint16(offset, littleEndian);
    if (tag !== 256 && tag !== 257) continue;
    const type = view.getUint16(offset + 2, littleEndian);
    const itemCount = view.getUint32(offset + 4, littleEndian);
    if (itemCount !== 1 || (type !== 3 && type !== 4)) continue;
    const value =
      type === 3
        ? view.getUint16(offset + 8, littleEndian)
        : view.getUint32(offset + 8, littleEndian);
    if (tag === 256) width = value;
    else height = value;
  }
  return width && height ? { width, height } : undefined;
}

function jpegDimensions(
  data: Uint8Array,
): { width: number; height: number } | undefined {
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  for (let offset = 2; offset + 9 < data.byteLength;) {
    if (data[offset] !== 0xff) return undefined;
    const marker = data[offset + 1]!;
    const length = view.getUint16(offset + 2);
    if (marker >= 0xc0 && marker <= 0xc3)
      return {
        width: view.getUint16(offset + 7),
        height: view.getUint16(offset + 5),
      };
    if (length < 2) return undefined;
    offset += 2 + length;
  }
  return undefined;
}

function isZipOffice(format: DocumentFormat): boolean {
  return ["docx", "docm", "xlsx", "xlsm", "pptx", "pptm", "ppsx"].includes(
    format,
  );
}

function ascii(data: Uint8Array, start: number, end: number): string {
  return String.fromCharCode(...data.subarray(start, end));
}

function limitError(
  message: string,
  actual: number,
  limit: number,
): ViewerError {
  return new ViewerError("resource-limit", message, {
    details: { actual, limit },
  });
}
