import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { performance } from "node:perf_hooks";

import {
  defaultResourceLimits,
  detectFormat,
  enforceContainerLimits,
  parseDelimitedBytes,
  sanitizeSvg,
} from "../packages/viewer/dist/index.js";

const root = resolve(import.meta.dirname, "..");
const iterations = Number(process.env.FUZZ_ITERATIONS ?? 2_000);
const maxCaseMs = Number(process.env.FUZZ_CASE_MS ?? 100);
const seeds = [
  bytes("%PDF-1.7\n1 0 obj<<>>endobj"),
  Uint8Array.of(0x50, 0x4b, 0x03, 0x04, 0, 0, 0, 0),
  Uint8Array.of(0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1),
  bytes('<svg xmlns="http://www.w3.org/2000/svg"><script>x</script></svg>'),
  bytes('a,b\n"quoted\nfield",c'),
  Uint8Array.of(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a),
];

let randomState = 0x5eed1234;
let slowestMs = 0;
let failures = 0;
const startedAt = performance.now();

for (let iteration = 0; iteration < iterations; iteration += 1) {
  const sample = mutate(seeds[iteration % seeds.length]);
  const caseStarted = performance.now();
  exercise(sample);
  const elapsed = performance.now() - caseStarted;
  slowestMs = Math.max(slowestMs, elapsed);
  if (elapsed > maxCaseMs) {
    failures += 1;
    throw new Error(
      `Mutation ${iteration} exceeded ${maxCaseMs} ms (${elapsed.toFixed(2)} ms)`,
    );
  }
}

const report = {
  schemaVersion: 1,
  seed: "0x5eed1234",
  iterations,
  failures,
  elapsedMs: Number((performance.now() - startedAt).toFixed(2)),
  slowestCaseMs: Number(slowestMs.toFixed(2)),
  targets: ["detection/ZIP/CFB", "SVG sanitizer", "CSV/TSV parser"],
};
await mkdir(resolve(root, "artifacts"), { recursive: true });
await writeFile(
  resolve(root, "artifacts/fuzz-js.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
console.log("JavaScript mutation fuzz passed", report);

function exercise(sample) {
  try {
    const detection = detectFormat(sample, { fileName: "mutation.csv" });
    try {
      enforceContainerLimits(sample, detection.format, {
        ...defaultResourceLimits,
        maxInputBytes: 2 * 1024 * 1024,
      });
    } catch {}
  } catch {}
  const text = new TextDecoder("utf-8", { fatal: false }).decode(sample);
  try {
    sanitizeSvg(text);
  } catch {}
  for (const format of ["csv", "tsv"])
    try {
      parseDelimitedBytes(sample, format, 10_000);
    } catch {}
}

function mutate(seed) {
  let output = seed.slice();
  const operations = 1 + (random() % 8);
  for (let operation = 0; operation < operations; operation += 1) {
    const choice = random() % 4;
    if (choice === 0 && output.length > 0) {
      output[random() % output.length] ^= random() & 0xff;
    } else if (choice === 1 && output.length > 0) {
      output = output.slice(0, random() % output.length);
    } else if (choice === 2 && output.length < 64 * 1024) {
      const extra = new Uint8Array(1 + (random() % 128));
      for (let index = 0; index < extra.length; index += 1)
        extra[index] = random() & 0xff;
      const joined = new Uint8Array(output.length + extra.length);
      joined.set(output);
      joined.set(extra, output.length);
      output = joined;
    } else if (output.length > 0 && output.length < 32 * 1024) {
      const joined = new Uint8Array(output.length * 2);
      joined.set(output);
      joined.set(output, output.length);
      output = joined;
    }
  }
  return output;
}

function random() {
  randomState = (Math.imul(randomState, 1_664_525) + 1_013_904_223) >>> 0;
  return randomState;
}

function bytes(value) {
  return new TextEncoder().encode(value);
}
