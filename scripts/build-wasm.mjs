import { execFileSync } from "node:child_process";
import { access, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import binaryen from "binaryen";

const root = resolve(import.meta.dirname, "..");
const toolRoot = resolve(root, ".tools");
const wasmBindgen = resolve(toolRoot, "bin/wasm-bindgen");
const target = resolve(root, "target/wasm32-unknown-unknown/release");
const outputRoot = resolve(root, ".cache/wasm-bindgen");

try {
  await access(wasmBindgen);
} catch {
  execFileSync(
    "cargo",
    [
      "install",
      "wasm-bindgen-cli",
      "--version",
      "0.2.126",
      "--locked",
      "--root",
      toolRoot,
    ],
    { cwd: root, stdio: "inherit" },
  );
}

execFileSync(
  "cargo",
  [
    "build",
    "--release",
    "--target",
    "wasm32-unknown-unknown",
    "-p",
    "legacy-office-wasm",
    "-p",
    "pdf-wasm",
    "-p",
    "image-wasm",
  ],
  { cwd: root, stdio: "inherit" },
);

for (const [name, fileName] of [
  ["legacy", "legacy_office_wasm.wasm"],
  ["pdf", "pdf_wasm.wasm"],
  ["image", "image_wasm.wasm"],
]) {
  const outdir = resolve(outputRoot, name);
  await mkdir(outdir, { recursive: true });
  const inputPath = resolve(target, fileName);
  const wasmPath = resolve(outdir, "index_bg.wasm");
  try {
    const [inputMetadata, outputMetadata] = await Promise.all([
      stat(inputPath),
      stat(wasmPath),
    ]);
    await access(resolve(outdir, "index.js"));
    if (outputMetadata.mtimeMs >= inputMetadata.mtimeMs) continue;
  } catch {
    // Missing or stale output is regenerated below.
  }
  execFileSync(
    wasmBindgen,
    [inputPath, "--target", "web", "--out-dir", outdir, "--out-name", "index"],
    { cwd: root, stdio: "inherit" },
  );

  binaryen.setOptimizeLevel(4);
  binaryen.setShrinkLevel(2);
  const module = binaryen.readBinary(await readFile(wasmPath));
  module.setFeatures(
    module.getFeatures() |
      binaryen.Features.BulkMemory |
      binaryen.Features.BulkMemoryOpt,
  );
  module.optimize();
  await writeFile(wasmPath, module.emitBinary());
  module.dispose();
}

console.log(`WASM browser bindings ready in ${outputRoot}`);
