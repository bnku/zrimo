import { execFileSync } from "node:child_process";
import { readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const listed = git([
  "ls-files",
  "--cached",
  "--others",
  "--exclude-standard",
  "-z",
])
  .split("\0")
  .filter(Boolean);
const tracked = [];
for (const path of listed) {
  try {
    await stat(resolve(root, path));
    tracked.push(path);
  } catch {
    // A tracked path deleted by the candidate is intentionally absent.
  }
}
const trackedSet = new Set(tracked);
const issues = [];

const requiredFiles = [
  ".github/workflows/ci.yml",
  ".github/workflows/pages.yml",
  ".github/workflows/publish.yml",
  "Cargo.lock",
  "Cargo.toml",
  "LICENSE-APACHE",
  "LICENSE-MIT",
  "README.md",
  "SECURITY.md",
  "docs/.vitepress/config.ts",
  "docs/index.md",
  "package-lock.json",
  "package.json",
  "packages/viewer/README.md",
  "packages/viewer/bin/copy-assets.mjs",
  "packages/viewer/package.json",
  "packages/viewer/src/index.ts",
];
for (const path of requiredFiles) {
  if (!trackedSet.has(path))
    issues.push(`required file is not tracked: ${path}`);
}

const forbiddenSegments = new Set([
  ".cache",
  ".tmp",
  "coverage",
  "node_modules",
  "playwright-report",
  "target",
  "test-results",
]);
for (const path of tracked) {
  const segments = path.split("/");
  if (segments.some((segment) => forbiddenSegments.has(segment))) {
    issues.push(`generated/private path is tracked: ${path}`);
  }
  if (/^artifacts\/.*\.(?:tgz|png)$/i.test(path)) {
    issues.push(`generated release binary is tracked: ${path}`);
  }
  if (/(?:^|\/)(?:\.env(?:\..*)?|id_rsa|.*\.pem|.*\.pfx)$/i.test(path)) {
    issues.push(`sensitive-looking path is tracked: ${path}`);
  }
}

const ignoredTracked = git(["ls-files", "-ci", "--exclude-standard"])
  .split("\n")
  .filter((path) => path && trackedSet.has(path));
for (const path of ignoredTracked)
  issues.push(`tracked file is ignored: ${path}`);

for (const path of tracked) {
  const metadata = await stat(resolve(root, path));
  const allowedFontPayload =
    /^packages\/viewer\/fonts\/[^/]+\.woff2$/.test(path) &&
    metadata.size <= 16 * 1024 * 1024;
  if (metadata.size > 8 * 1024 * 1024 && !allowedFontPayload) {
    issues.push(`tracked file exceeds 8 MiB: ${path}`);
  }
  if (metadata.size > 2 * 1024 * 1024) continue;
  if (!/\.(?:c?js|mjs|json|md|rs|toml|ts|tsx|yml|yaml)$/i.test(path)) continue;
  const content = await readFile(resolve(root, path), "utf8");
  if (/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(content)) {
    issues.push(`private key material found: ${path}`);
  }
  if (/\bgh[opsu]_[A-Za-z0-9]{36,}\b/.test(content)) {
    issues.push(`GitHub token-shaped value found: ${path}`);
  }
  if (/\bAKIA[0-9A-Z]{16}\b/.test(content)) {
    issues.push(`AWS key-shaped value found: ${path}`);
  }
}

for (const path of [
  "node_modules/probe",
  ".cache/probe",
  ".tmp/probe.pdf",
  "target/probe",
  "packages/viewer/dist/index.js",
  "docs/.vitepress/dist/index.html",
  "artifacts/release/probe.tgz",
]) {
  if (!isIgnored(path)) issues.push(`generated path is not ignored: ${path}`);
}
for (const path of [
  "packages/viewer/src/index.ts",
  "examples/react/src/main.tsx",
  "docs/getting-started.md",
  ".github/workflows/publish.yml",
]) {
  if (isIgnored(path)) issues.push(`distribution source is ignored: ${path}`);
}

const packageManifest = JSON.parse(
  await readFile(resolve(root, "packages/viewer/package.json"), "utf8"),
);
const releaseStatus = JSON.parse(
  await readFile(resolve(root, "release-status.json"), "utf8"),
);
if (packageManifest.version !== releaseStatus.packageVersion) {
  issues.push("package and release-status versions differ");
}
if (!/^\d+\.\d+\.\d+$/.test(packageManifest.version)) {
  issues.push(
    `viewer version is not stable semver: ${packageManifest.version}`,
  );
}
for (const file of [
  "bin",
  "dist",
  "README.md",
  "LICENSE-MIT",
  "LICENSE-APACHE",
  "THIRD_PARTY_NOTICES.md",
]) {
  if (!packageManifest.files?.includes(file)) {
    issues.push(`npm files allowlist omits ${file}`);
  }
}

const origin = normalizeRepository(
  git(["config", "--get", "remote.origin.url"]).trim(),
);
const declared = normalizeRepository(packageManifest.repository?.url ?? "");
if (!origin || origin !== declared) {
  issues.push(
    `package repository does not match origin: ${declared || "missing"} != ${origin || "missing"}`,
  );
}

if (issues.length) {
  throw new Error(`Repository audit failed:\n- ${issues.join("\n- ")}`);
}
console.log(
  `Repository audit passed: ${tracked.length} candidate files, no ignored/private/generated release payloads.`,
);

function git(args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" });
}

function isIgnored(path) {
  try {
    execFileSync("git", ["check-ignore", "--no-index", "--quiet", path], {
      cwd: root,
      stdio: "ignore",
    });
    return true;
  } catch {
    return false;
  }
}

function normalizeRepository(value) {
  return String(value)
    .trim()
    .replace(/^git@github\.com:/, "https://github.com/")
    .replace(/^git\+/, "")
    .replace(/\/$/, "")
    .replace(/\.git$/, "")
    .toLowerCase();
}
