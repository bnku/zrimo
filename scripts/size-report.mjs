import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { extname, relative, resolve } from "node:path";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

const root = resolve(import.meta.dirname, "..");
const dist = resolve(root, "packages/viewer/dist");
const wasmArtifacts = [
  ["ooxml-docx", "node_modules/@silurus/ooxml/dist/docx_parser_bg.wasm"],
  ["ooxml-xlsx", "node_modules/@silurus/ooxml/dist/xlsx_parser_bg.wasm"],
  ["ooxml-pptx", "node_modules/@silurus/ooxml/dist/pptx_parser_bg.wasm"],
  ["legacy-office", "packages/viewer/dist/assets/legacy/index_bg.wasm"],
  ["tiff-image", "packages/viewer/dist/assets/image/index_bg.wasm"],
];

const files = await walk(dist);
const codePaths = files.filter((path) => {
  const name = relative(dist, path);
  return (
    !name.startsWith("assets/pdfjs/") &&
    name !== "workers/pdf.worker.min.mjs" &&
    !name.startsWith("fonts/") &&
    !name.endsWith(".map") &&
    !name.endsWith(".d.ts") &&
    !name.endsWith(".wasm") &&
    [".js", ".css", ".json"].includes(extname(name))
  );
});
const pdfPaths = files.filter((path) => {
  const name = relative(dist, path);
  return (
    name.startsWith("assets/pdfjs/") || name === "workers/pdf.worker.min.mjs"
  );
});
const fontPaths = files.filter(
  (path) =>
    relative(dist, path).startsWith("fonts/") &&
    [".woff2", ".json", ".txt", ".md"].includes(extname(path)),
);

const assets = [];
for (const path of codePaths)
  assets.push(await measure(`code/${relative(dist, path)}`, path, "code"));
for (const path of pdfPaths)
  assets.push(await measure(`pdfjs/${relative(dist, path)}`, path, "pdf"));
assets.push(
  await measure(
    "pdfjs/dependency/build/pdf.mjs",
    resolve(root, "node_modules/pdfjs-dist/build/pdf.mjs"),
    "pdf",
  ),
);
for (const [name, path] of wasmArtifacts)
  assets.push(await measure(`wasm/${name}`, resolve(root, path), "wasm"));
for (const path of fontPaths)
  assets.push(
    await measure(
      `font/${relative(resolve(dist, "fonts"), path)}`,
      path,
      "font",
    ),
  );

const base = sum(assets.filter((asset) => asset.group !== "font"));
const fonts = sum(assets.filter((asset) => asset.group === "font"));
const report = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  thresholds: {
    targetBrotliBytes: 20 * 1024 * 1024,
    releaseBlockBrotliBytes: 25 * 1024 * 1024,
    optionalFontsExcluded: true,
  },
  status:
    base.brotli > 25 * 1024 * 1024
      ? "blocked"
      : base.brotli > 20 * 1024 * 1024
        ? "explanation-required"
        : "pass",
  totals: {
    base,
    optionalFonts: fonts,
    deliveredWithAllFonts: add(base, fonts),
  },
  assets,
};

console.table(
  [
    { name: "BASE code + WASM", ...base },
    { name: "OPTIONAL fonts", ...fonts },
    { name: "ALL", ...add(base, fonts) },
  ].map((entry) => ({
    ...entry,
    rawMiB: mib(entry.raw),
    gzipMiB: mib(entry.gzip),
    brotliMiB: mib(entry.brotli),
  })),
);
console.log(`Size gate: ${report.status}`);
await mkdir(resolve(root, "artifacts"), { recursive: true });
await writeFile(
  resolve(root, "artifacts/size-report.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
if (report.status === "blocked") process.exitCode = 1;

async function walk(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) result.push(...(await walk(path)));
    else result.push(path);
  }
  return result;
}

async function measure(name, path, group) {
  const bytes = await readFile(path);
  return {
    name,
    group,
    raw: bytes.byteLength,
    gzip: gzipSync(bytes, { level: 9 }).byteLength,
    brotli: brotliCompressSync(bytes, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).byteLength,
  };
}

function sum(entries) {
  return entries.reduce((total, entry) => add(total, entry), {
    raw: 0,
    gzip: 0,
    brotli: 0,
  });
}

function add(left, right) {
  return {
    raw: left.raw + right.raw,
    gzip: left.gzip + right.gzip,
    brotli: left.brotli + right.brotli,
  };
}

function mib(bytes) {
  return (bytes / 1024 / 1024).toFixed(2);
}
