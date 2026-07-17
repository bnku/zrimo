import { gunzipSync, inflateRawSync } from "node:zlib";

const MAX_DEPTH = 4;
const MAX_ENTRY_COUNT = 20_000;
const MAX_EXPANDED_BYTES = 256 * 1024 * 1024;
const forbiddenSegments = new Set([
  ".cache",
  ".env",
  ".git",
  ".tmp",
  "corpus",
  "fixture",
  "fixtures",
  "playwright-report",
  "private",
  "src",
  "temp",
  "test",
  "test-results",
  "tests",
  "tmp",
]);
const forbiddenExtensions = new Set([
  ".7z",
  ".doc",
  ".docm",
  ".docx",
  ".env",
  ".gif",
  ".gz",
  ".jpeg",
  ".jpg",
  ".key",
  ".map",
  ".pdf",
  ".pem",
  ".pfx",
  ".png",
  ".ppt",
  ".pptm",
  ".pptx",
  ".rar",
  ".tar",
  ".tgz",
  ".tif",
  ".tiff",
  ".xls",
  ".xlsm",
  ".xlsx",
  ".zip",
]);

export function assertPackedFileListSafe(files) {
  const issues = [];
  for (const file of files) {
    const path = normalizePath(file.path ?? file);
    if (!isAllowedPackagePath(path)) issues.push(`not-allowlisted:${path}`);
    issues.push(...pathIssues(path));
  }
  if (issues.length) throw policyError(issues);
}

export function assertPackedTarballSafe(bytes, options = {}) {
  const state = {
    entries: 0,
    expandedBytes: 0,
    issues: [],
    sentinel: options.sentinel ? Buffer.from(options.sentinel) : undefined,
  };
  const archive = toBuffer(bytes);
  const tar = isGzip(archive) ? safeGunzip(archive, state, "package") : archive;
  const entries = readTar(tar, state, "package");
  for (const entry of entries) {
    const path = normalizePath(stripPackagePrefix(entry.name));
    if (!path) continue;
    if (!isAllowedPackagePath(path)) {
      state.issues.push(`not-allowlisted:${path}`);
    }
    state.issues.push(...pathIssues(path));
    if (entry.type === "file") scanPayload(entry.bytes, path, 0, state);
  }
  if (state.issues.length) throw policyError(state.issues);
  return { entryCount: state.entries, expandedBytes: state.expandedBytes };
}

function scanPayload(bytes, path, depth, state) {
  account(bytes, state);
  if (state.sentinel && bytes.indexOf(state.sentinel) >= 0) {
    state.issues.push(`sentinel-content:${path}`);
  }
  const signature = forbiddenSignature(bytes);
  if (signature) state.issues.push(`forbidden-${signature}:${path}`);
  if (depth >= MAX_DEPTH) {
    if (isArchive(bytes)) state.issues.push(`archive-depth:${path}`);
    return;
  }
  if (isZip(bytes)) {
    state.issues.push(`embedded-archive:${path}`);
    for (const entry of readZip(bytes, state, path)) {
      const nested = normalizePath(entry.name);
      state.issues.push(...pathIssues(`${path}!/${nested}`));
      scanPayload(entry.bytes, `${path}!/${nested}`, depth + 1, state);
    }
  } else if (isGzip(bytes)) {
    state.issues.push(`embedded-archive:${path}`);
    scanPayload(
      safeGunzip(bytes, state, path),
      `${path}!/gzip`,
      depth + 1,
      state,
    );
  } else if (isTar(bytes)) {
    state.issues.push(`embedded-archive:${path}`);
    for (const entry of readTar(bytes, state, path)) {
      const nested = normalizePath(entry.name);
      state.issues.push(...pathIssues(`${path}!/${nested}`));
      if (entry.type === "file") {
        scanPayload(entry.bytes, `${path}!/${nested}`, depth + 1, state);
      }
    }
  }
}

function readTar(bytes, state, context) {
  const entries = [];
  let offset = 0;
  while (offset + 512 <= bytes.length) {
    const header = bytes.subarray(offset, offset + 512);
    if (header.every((value) => value === 0)) break;
    const name = readTarString(header, 0, 100);
    const prefix = readTarString(header, 345, 155);
    const fullName = prefix ? `${prefix}/${name}` : name;
    const sizeText = readTarString(header, 124, 12).trim();
    const size = sizeText ? Number.parseInt(sizeText, 8) : 0;
    if (!Number.isSafeInteger(size) || size < 0) {
      throw policyError([`invalid-tar-size:${context}`]);
    }
    const typeByte = header[156];
    const type = typeByte === 0 || typeByte === 48 ? "file" : "other";
    const start = offset + 512;
    const end = start + size;
    if (end > bytes.length) throw policyError([`truncated-tar:${context}`]);
    countEntry(state, size);
    entries.push({ name: fullName, type, bytes: bytes.subarray(start, end) });
    offset = start + Math.ceil(size / 512) * 512;
  }
  return entries;
}

function readZip(bytes, state, context) {
  const eocd = findSignatureBackwards(bytes, 0x06054b50);
  if (eocd < 0 || eocd + 22 > bytes.length) {
    throw policyError([`invalid-zip:${context}`]);
  }
  const total = bytes.readUInt16LE(eocd + 10);
  const centralOffset = bytes.readUInt32LE(eocd + 16);
  if (total === 0xffff || centralOffset === 0xffffffff) {
    throw policyError([`zip64-unsupported:${context}`]);
  }
  const entries = [];
  let offset = centralOffset;
  for (let index = 0; index < total; index += 1) {
    if (
      offset + 46 > bytes.length ||
      bytes.readUInt32LE(offset) !== 0x02014b50
    ) {
      throw policyError([`invalid-zip-directory:${context}`]);
    }
    const flags = bytes.readUInt16LE(offset + 8);
    const method = bytes.readUInt16LE(offset + 10);
    const compressedSize = bytes.readUInt32LE(offset + 20);
    const expandedSize = bytes.readUInt32LE(offset + 24);
    const nameLength = bytes.readUInt16LE(offset + 28);
    const extraLength = bytes.readUInt16LE(offset + 30);
    const commentLength = bytes.readUInt16LE(offset + 32);
    const localOffset = bytes.readUInt32LE(offset + 42);
    const nameStart = offset + 46;
    const nameEnd = nameStart + nameLength;
    if (nameEnd > bytes.length || localOffset + 30 > bytes.length) {
      throw policyError([`truncated-zip:${context}`]);
    }
    const name = bytes.subarray(nameStart, nameEnd).toString("utf8");
    if (flags & 1) throw policyError([`encrypted-zip:${context}`]);
    if (bytes.readUInt32LE(localOffset) !== 0x04034b50) {
      throw policyError([`invalid-zip-entry:${context}`]);
    }
    const localNameLength = bytes.readUInt16LE(localOffset + 26);
    const localExtraLength = bytes.readUInt16LE(localOffset + 28);
    const dataStart = localOffset + 30 + localNameLength + localExtraLength;
    const dataEnd = dataStart + compressedSize;
    if (dataEnd > bytes.length) throw policyError([`truncated-zip:${context}`]);
    const compressed = bytes.subarray(dataStart, dataEnd);
    let expanded;
    if (method === 0) expanded = compressed;
    else if (method === 8) expanded = inflateRawSync(compressed);
    else throw policyError([`zip-method-${method}:${context}`]);
    if (expanded.length !== expandedSize) {
      throw policyError([`zip-size-mismatch:${context}`]);
    }
    countEntry(state, expanded.length);
    entries.push({ name, bytes: expanded });
    offset = nameEnd + extraLength + commentLength;
  }
  return entries;
}

function pathIssues(path) {
  const issues = [];
  const normalized = normalizePath(path);
  const segments = normalized.toLowerCase().split(/[!/]+/).filter(Boolean);
  if (
    !normalized ||
    normalized.startsWith("/") ||
    normalized.includes("\\") ||
    segments.includes("..") ||
    /[\0-\x1f]/.test(normalized)
  ) {
    issues.push(`unsafe-path:${normalized || "<empty>"}`);
  }
  for (const segment of segments) {
    if (forbiddenSegments.has(segment)) {
      issues.push(`forbidden-segment:${normalized}`);
      break;
    }
  }
  const lower = normalized.toLowerCase();
  for (const extension of forbiddenExtensions) {
    if (lower.endsWith(extension)) {
      issues.push(`forbidden-extension:${normalized}`);
      break;
    }
  }
  if (/(?:secret|credential|private[-_]?key)/i.test(normalized)) {
    issues.push(`sensitive-name:${normalized}`);
  }
  return issues;
}

function isAllowedPackagePath(path) {
  return (
    path === "package.json" ||
    path === "README.md" ||
    path === "LICENSE-MIT" ||
    path === "LICENSE-APACHE" ||
    path === "THIRD_PARTY_NOTICES.md" ||
    path === "bin/copy-assets.mjs" ||
    path.startsWith("dist/")
  );
}

function forbiddenSignature(bytes) {
  if (bytes.subarray(0, 5).toString("ascii") === "%PDF-") return "pdf";
  if (bytes.subarray(0, 8).equals(Buffer.from("d0cf11e0a1b11ae1", "hex"))) {
    return "ole";
  }
  if (bytes.subarray(0, 8).equals(Buffer.from("89504e470d0a1a0a", "hex"))) {
    return "png";
  }
  if (bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff)
    return "jpeg";
  if (["GIF87a", "GIF89a"].includes(bytes.subarray(0, 6).toString("ascii"))) {
    return "gif";
  }
  if (
    bytes.subarray(0, 4).equals(Buffer.from("49492a00", "hex")) ||
    bytes.subarray(0, 4).equals(Buffer.from("4d4d002a", "hex"))
  ) {
    return "tiff";
  }
  return undefined;
}

function safeGunzip(bytes, state, context) {
  try {
    const expanded = gunzipSync(bytes, { maxOutputLength: MAX_EXPANDED_BYTES });
    account(expanded, state);
    return expanded;
  } catch {
    throw policyError([`invalid-gzip:${context}`]);
  }
}

function account(bytes, state) {
  state.expandedBytes += bytes.length;
  if (state.expandedBytes > MAX_EXPANDED_BYTES) {
    throw policyError(["expanded-size-limit"]);
  }
}

function countEntry(state, bytes) {
  state.entries += 1;
  if (state.entries > MAX_ENTRY_COUNT) throw policyError(["entry-count-limit"]);
  if (bytes > MAX_EXPANDED_BYTES) throw policyError(["entry-size-limit"]);
}

function isArchive(bytes) {
  return isZip(bytes) || isGzip(bytes) || isTar(bytes);
}

function isZip(bytes) {
  return bytes.length >= 4 && bytes.readUInt32LE(0) === 0x04034b50;
}

function isGzip(bytes) {
  return bytes[0] === 0x1f && bytes[1] === 0x8b;
}

function isTar(bytes) {
  return (
    bytes.length >= 512 &&
    bytes.subarray(257, 262).toString("ascii") === "ustar"
  );
}

function findSignatureBackwards(bytes, signature) {
  for (
    let offset = bytes.length - 22;
    offset >= Math.max(0, bytes.length - 65_557);
    offset -= 1
  ) {
    if (bytes.readUInt32LE(offset) === signature) return offset;
  }
  return -1;
}

function readTarString(bytes, offset, length) {
  const end = bytes.indexOf(0, offset);
  const boundedEnd = end < 0 || end > offset + length ? offset + length : end;
  return bytes.subarray(offset, boundedEnd).toString("utf8");
}

function stripPackagePrefix(path) {
  return path.startsWith("package/") ? path.slice("package/".length) : path;
}

function normalizePath(path) {
  return String(path)
    .replace(/^\.\//, "")
    .replace(/\/{2,}/g, "/");
}

function toBuffer(bytes) {
  return Buffer.isBuffer(bytes)
    ? bytes
    : Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength);
}

function policyError(issues) {
  const unique = [...new Set(issues)].slice(0, 20);
  return new Error(
    `Packed content violates release policy: ${unique.join(", ")}`,
  );
}
