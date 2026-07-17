import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import test from "node:test";

const root = resolve(import.meta.dirname, "..");
const harness = resolve(root, "scripts/regression-harness.mjs");

test("skips the private lane when it is not configured", () => {
  const environment = { ...process.env };
  delete environment.PRIVATE_REGRESSION_DIR;
  const output = execFileSync("node", [harness], {
    cwd: root,
    env: environment,
    encoding: "utf8",
  });
  assert.match(output, /private lane skipped/);
});

test("reports only a case id and leaves private inputs unchanged", async () => {
  const directory = await mkdtemp(
    resolve(tmpdir(), "viewer-private-regression-"),
  );
  try {
    await writeFile(resolve(directory, "input.bin"), "%PDF-1.7\n");
    await writeFile(
      resolve(directory, "manifest.json"),
      `${JSON.stringify(
        {
          schemaVersion: 1,
          cases: [{ id: "local-case-a", family: "pdf", file: "input.bin" }],
        },
        null,
        2,
      )}\n`,
    );
    const before = await snapshot(directory);
    const result = spawnSync("node", [harness], {
      cwd: root,
      env: { ...process.env, PRIVATE_REGRESSION_DIR: directory },
      encoding: "utf8",
    });
    const after = await snapshot(directory);
    assert.equal(result.status, 1);
    assert.equal(result.stdout.trim(), "local-case-a: fail");
    assert.equal(result.stderr, "");
    assert.deepEqual(after, before);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

async function snapshot(directory) {
  const names = (await readdir(directory)).sort();
  const files = await Promise.all(
    names.map(async (name) => [
      name,
      await readFile(resolve(directory, name), "hex"),
    ]),
  );
  return Object.fromEntries(files);
}
