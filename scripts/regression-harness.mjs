import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { isAbsolute, relative, resolve, sep } from "node:path";

const root = resolve(import.meta.dirname, "..");
const manifest = await readJson(
  resolve(root, "tests/regressions/manifest.json"),
  "Committed regression manifest is invalid.",
);
await validateCommittedManifest(manifest);
await validateReleaseState(manifest);

const privateRootValue = process.env.PRIVATE_REGRESSION_DIR;
if (!privateRootValue) {
  console.log(
    `Regression manifest valid: ${manifest.cases.length} cases; private lane skipped.`,
  );
  process.exit(0);
}

const privateRoot = resolve(privateRootValue);
assertPrivateLocation(privateRoot);
const privateManifest = await readJson(
  resolve(privateRoot, "manifest.json"),
  "Private regression manifest is invalid.",
);
const privateCases = validatePrivateManifest(privateManifest);
const gates = new Map(manifest.cases.map((entry) => [entry.family, entry]));
let failed = false;

for (const entry of privateCases) {
  let passed = false;
  try {
    const fixturePath = resolvePrivateFile(privateRoot, entry.file);
    const bytes = await readFile(fixturePath);
    validateSignature(entry.family, bytes);
    const gate = gates.get(entry.family);
    if (gate?.status === "pass" && typeof gate.privateRunner === "string") {
      const runner = resolve(root, gate.privateRunner);
      assertInside(root, runner);
      execFileSync("node", [runner], {
        stdio: "ignore",
        env: {
          ...process.env,
          PRIVATE_REGRESSION_CASE_ID: entry.id,
          PRIVATE_REGRESSION_FAMILY: entry.family,
          PRIVATE_REGRESSION_FILE: fixturePath,
        },
      });
      passed = true;
    }
  } catch {
    passed = false;
  }
  console.log(`${entry.id}: ${passed ? "pass" : "fail"}`);
  failed ||= !passed;
}

if (failed) process.exitCode = 1;

async function validateCommittedManifest(value) {
  if (value?.schemaVersion !== 1 || !Array.isArray(value.cases)) {
    throw new Error("Committed regression manifest is invalid.");
  }
  const ids = new Set();
  for (const entry of value.cases) {
    if (
      !isCaseId(entry.id) ||
      ids.has(entry.id) ||
      !["docx", "xlsx", "pdf", "doc"].includes(entry.family) ||
      !["pass", "unsupported"].includes(entry.status) ||
      typeof entry.oracle !== "string" ||
      !["public-corpus", "generated"].includes(entry.source?.kind)
    ) {
      throw new Error("Committed regression manifest is invalid.");
    }
    ids.add(entry.id);
    if (entry.status === "unsupported" && !/^1[0-3]$/.test(entry.blockedBy)) {
      throw new Error("Unsupported regression gate has no corrective task.");
    }
    if (entry.status === "pass") {
      if (typeof entry.runner !== "string") {
        throw new Error("Passing regression gate has no executable runner.");
      }
      const runner = resolve(root, entry.runner);
      assertInside(root, runner);
      try {
        await readFile(runner);
      } catch {
        throw new Error("Passing regression runner does not exist.");
      }
    }
    if (entry.source.kind === "public-corpus") {
      if (
        typeof entry.source.manifest !== "string" ||
        typeof entry.source.entry !== "string"
      )
        throw new Error("Committed regression manifest is invalid.");
      const corpusPath = resolve(root, entry.source.manifest);
      if (entry.source.manifest !== "tests/corpus/manifest.json") {
        throw new Error("Regression fixture provenance is not authoritative.");
      }
      assertInside(root, corpusPath);
      const corpus = await readJson(
        corpusPath,
        "Corpus provenance is invalid.",
      );
      const fixture = corpus.files?.find(
        (candidate) => candidate.name === entry.source.entry,
      );
      if (
        !fixture ||
        fixture.format !== entry.family ||
        typeof fixture.sourceRepository !== "string" ||
        !/^[a-f0-9]{40}$/.test(fixture.sourceCommit) ||
        typeof fixture.sourcePath !== "string" ||
        ![
          "Apache-2.0",
          "BSD-2-Clause",
          "BSD-3-Clause",
          "CC0-1.0",
          "MIT",
        ].includes(fixture.license) ||
        !/^[a-f0-9]{64}$/.test(fixture.sha256)
      ) {
        throw new Error("Regression fixture provenance is incomplete.");
      }
    } else if (
      typeof entry.source.generator !== "string" ||
      entry.source.generator !== entry.runner
    ) {
      throw new Error("Generated regression has no auditable runner.");
    }
  }
}

async function validateReleaseState(regressions) {
  const status = await readJson(
    resolve(root, "release-status.json"),
    "Release status is invalid.",
  );
  if (
    status.schemaVersion !== 1 ||
    !["blocked", "ready"].includes(status.state) ||
    typeof status.packageVersion !== "string" ||
    typeof status.reason !== "string" ||
    !Array.isArray(status.blockedChannels) ||
    !Array.isArray(status.requiredTasks)
  ) {
    throw new Error("Release status is invalid.");
  }
  const viewerPackage = await readJson(
    resolve(root, "packages/viewer/package.json"),
    "Viewer package manifest is invalid.",
  );
  if (status.packageVersion !== viewerPackage.version) {
    throw new Error("Release status version does not match the package.");
  }
  const unsupported = regressions.cases.filter(
    (entry) => entry.status === "unsupported",
  );
  if (unsupported.length > 0 && status.state !== "blocked") {
    throw new Error("Unsupported regressions require a blocked release state.");
  }
  const required = new Set(status.requiredTasks ?? []);
  if (unsupported.some((entry) => !required.has(entry.blockedBy))) {
    throw new Error("Release status omits a corrective blocker.");
  }
  if (
    status.state === "ready" &&
    (unsupported.length > 0 || status.blockedChannels?.length > 0)
  ) {
    throw new Error("Ready release status conflicts with active blockers.");
  }
}

function validatePrivateManifest(value) {
  if (value?.schemaVersion !== 1 || !Array.isArray(value.cases)) {
    throw new Error("Private regression manifest is invalid.");
  }
  const ids = new Set();
  for (const entry of value.cases) {
    if (
      !isCaseId(entry.id) ||
      ids.has(entry.id) ||
      !["docx", "xlsx", "pdf", "doc"].includes(entry.family) ||
      typeof entry.file !== "string"
    ) {
      throw new Error("Private regression manifest is invalid.");
    }
    ids.add(entry.id);
  }
  return value.cases;
}

function assertPrivateLocation(directory) {
  if (
    isInside(root, directory) &&
    !isInside(resolve(root, ".tmp"), directory)
  ) {
    throw new Error("Private regression directory is in release scope.");
  }
  const forbiddenRoots = [
    "artifacts",
    "docs",
    "examples",
    "packages",
    "scripts",
    "tests",
  ].map((path) => resolve(root, path));
  if (forbiddenRoots.some((path) => isInside(path, directory))) {
    throw new Error("Private regression directory is in release scope.");
  }
}

function resolvePrivateFile(directory, file) {
  if (isAbsolute(file)) {
    throw new Error("Private fixture paths must be relative.");
  }
  const path = resolve(directory, file);
  assertInside(directory, path);
  return path;
}

function validateSignature(family, bytes) {
  const zip = bytes
    .subarray(0, 4)
    .equals(Buffer.from([0x50, 0x4b, 0x03, 0x04]));
  const ole = bytes
    .subarray(0, 8)
    .equals(Buffer.from([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]));
  const pdf = bytes.subarray(0, 5).toString("ascii") === "%PDF-";
  const valid =
    (family === "pdf" && pdf) ||
    (family === "doc" && ole) ||
    (["docx", "xlsx"].includes(family) && zip);
  if (!valid)
    throw new Error("Private fixture signature does not match family.");
}

function isCaseId(value) {
  return typeof value === "string" && /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value);
}

function assertInside(parent, child) {
  if (!isInside(parent, child))
    throw new Error("Path escapes its allowed root.");
}

function isInside(parent, child) {
  const path = relative(parent, child);
  return path === "" || (!path.startsWith(`..${sep}`) && path !== "..");
}

async function readJson(path, message) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    throw new Error(message);
  }
}
