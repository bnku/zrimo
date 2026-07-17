import { spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const targets = [
  "format_detection",
  "legacy_doc",
  "legacy_office",
  "pdf_parser",
  "tiff_parser",
];
const budgetSeconds = Number(process.env.FUZZ_SECONDS ?? 10);
if (!Number.isFinite(budgetSeconds) || budgetSeconds <= 0)
  throw new Error(`Invalid FUZZ_SECONDS: ${process.env.FUZZ_SECONDS}`);
await seedCleanCheckoutCorpus();
const results = [];
for (const target of targets) {
  const started = performance.now();
  const result = spawnSync(
    "cargo",
    [
      "+nightly",
      "fuzz",
      "run",
      target,
      "--fuzz-dir",
      "fuzz",
      "--",
      `-max_total_time=${Math.trunc(budgetSeconds)}`,
    ],
    { cwd: root, stdio: "inherit" },
  );
  results.push({
    target,
    elapsedMs: performance.now() - started,
    status: result.status,
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
await mkdir(resolve(root, "artifacts"), { recursive: true });
await writeFile(
  resolve(root, "artifacts/fuzz-rust.json"),
  `${JSON.stringify(
    {
      schemaVersion: 1,
      budgetSecondsPerTarget: budgetSeconds,
      crashes: 0,
      results,
    },
    null,
    2,
  )}\n`,
);
console.log(`Rust fuzz gate passed for ${targets.length} targets.`);

async function seedCleanCheckoutCorpus() {
  const seeds = {
    format_detection: [Buffer.from("%PDF-1.7\n%%EOF\n")],
    legacy_doc: [Buffer.from("d0cf11e0a1b11ae1", "hex")],
    legacy_office: [Buffer.from("d0cf11e0a1b11ae1", "hex")],
    pdf_parser: [
      Buffer.from("%PDF-1.7\n1 0 obj<</Type/Catalog>>endobj\n%%EOF\n"),
    ],
    tiff_parser: [Buffer.from("49492a000800000000000000", "hex")],
  };
  for (const [target, corpus] of Object.entries(seeds)) {
    const directory = resolve(root, "fuzz/corpus", target);
    await mkdir(directory, { recursive: true });
    for (const [index, bytes] of corpus.entries()) {
      await writeFile(resolve(directory, `zrimo-seed-${index}`), bytes);
    }
  }
}
