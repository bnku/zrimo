import assert from "node:assert/strict";
import test from "node:test";
import { gzipSync } from "node:zlib";

import {
  assertPackedFileListSafe,
  assertPackedTarballSafe,
} from "./release-content-policy.mjs";

test("accepts the package allowlist", () => {
  assert.doesNotThrow(() =>
    assertPackedFileListSafe([
      { path: "package.json" },
      { path: "dist/index.js" },
      { path: "README.md" },
      { path: "LICENSE-MIT" },
    ]),
  );
});

test("rejects private, corpus, document, and source-map paths", () => {
  for (const path of [
    "dist/private/input.bin",
    "dist/corpus/case.bin",
    "dist/input.pdf",
    "dist/index.js.map",
  ]) {
    assert.throws(() => assertPackedFileListSafe([{ path }]));
  }
});

test("scans archive payloads by signature and catches a sentinel", () => {
  const clean = gzipSync(
    tar([
      ["package/package.json", Buffer.from("{}")],
      ["package/dist/index.js", Buffer.from("export {}")],
    ]),
  );
  assert.doesNotThrow(() => assertPackedTarballSafe(clean));

  const sentinel = "synthetic-private-sentinel";
  const contaminated = gzipSync(
    tar([
      ["package/package.json", Buffer.from("{}")],
      ["package/dist/assets/data.bin", Buffer.from(sentinel)],
    ]),
  );
  assert.throws(() => assertPackedTarballSafe(contaminated, { sentinel }));
});

test("recursively inspects a disguised ZIP", () => {
  const nested = zip("private/document.pdf", Buffer.from("%PDF-1.7"));
  const packed = gzipSync(
    tar([
      ["package/package.json", Buffer.from("{}")],
      ["package/dist/assets/opaque.bin", nested],
    ]),
  );
  assert.throws(
    () => assertPackedTarballSafe(packed),
    /embedded-archive|forbidden-pdf/,
  );
});

function tar(files) {
  const parts = [];
  for (const [name, bytes] of files) {
    const header = Buffer.alloc(512);
    header.write(name, 0, 100, "utf8");
    header.write("0000644\0", 100, 8, "ascii");
    header.write("0000000\0", 108, 8, "ascii");
    header.write("0000000\0", 116, 8, "ascii");
    header.write(
      `${bytes.length.toString(8).padStart(11, "0")}\0`,
      124,
      12,
      "ascii",
    );
    header.write("00000000000\0", 136, 12, "ascii");
    header.fill(0x20, 148, 156);
    header[156] = 48;
    header.write("ustar\0", 257, 6, "ascii");
    header.write("00", 263, 2, "ascii");
    let checksum = 0;
    for (const value of header) checksum += value;
    header.write(
      `${checksum.toString(8).padStart(6, "0")}\0 `,
      148,
      8,
      "ascii",
    );
    parts.push(header, bytes, Buffer.alloc((512 - (bytes.length % 512)) % 512));
  }
  parts.push(Buffer.alloc(1024));
  return Buffer.concat(parts);
}

function zip(name, bytes) {
  const nameBytes = Buffer.from(name);
  const local = Buffer.alloc(30 + nameBytes.length);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt16LE(0, 6);
  local.writeUInt16LE(0, 8);
  local.writeUInt32LE(0, 14);
  local.writeUInt32LE(bytes.length, 18);
  local.writeUInt32LE(bytes.length, 22);
  local.writeUInt16LE(nameBytes.length, 26);
  nameBytes.copy(local, 30);

  const centralOffset = local.length + bytes.length;
  const central = Buffer.alloc(46 + nameBytes.length);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt16LE(0, 8);
  central.writeUInt16LE(0, 10);
  central.writeUInt32LE(0, 16);
  central.writeUInt32LE(bytes.length, 20);
  central.writeUInt32LE(bytes.length, 24);
  central.writeUInt16LE(nameBytes.length, 28);
  nameBytes.copy(central, 46);

  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(1, 8);
  eocd.writeUInt16LE(1, 10);
  eocd.writeUInt32LE(central.length, 12);
  eocd.writeUInt32LE(centralOffset, 16);
  return Buffer.concat([local, bytes, central, eocd]);
}
