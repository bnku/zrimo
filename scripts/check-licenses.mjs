import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const allowed = new Set([
  "0BSD",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "ISC",
  "MIT",
  "MIT-0",
  "OFL-1.1",
  "Unicode-3.0",
  "Unlicense",
  "Zlib",
]);
const forbidden = /(?:^|[^A-Z])(A?GPL|LGPL|MPL|EPL|CDDL|SSPL)(?:-|[^A-Z]|$)/i;

function accepted(expression) {
  if (!expression) return false;
  const alternatives = expression
    .replaceAll(/[()]/g, "")
    .split(/\s+OR\s+|\s*\/\s*/i);
  return alternatives.some((alternative) => {
    if (forbidden.test(alternative)) return false;
    const conjunction = alternative.split(/\s+AND\s+/i);
    return conjunction.every((part) => allowed.has(part.trim()));
  });
}

const failures = [];
const root = resolve(import.meta.dirname, "..");
const lock = JSON.parse(
  readFileSync(new URL("../package-lock.json", import.meta.url), "utf8"),
);
const npmProductionTree = JSON.parse(
  execFileSync(
    "npm",
    [
      "ls",
      "--workspace",
      "@docs-viewer-wasm/viewer",
      "--omit=dev",
      "--all",
      "--json",
    ],
    { encoding: "utf8" },
  ),
);
const npmProduction = new Set();
collectNpmProduction(npmProductionTree.dependencies ?? {});
for (const [path, metadata] of Object.entries(lock.packages ?? {})) {
  if (!path.startsWith("node_modules/") || metadata.link) continue;
  const name = metadata.name ?? path.slice("node_modules/".length);
  if (!npmProduction.has(`${name}@${metadata.version ?? "?"}`)) continue;
  if (!accepted(metadata.license))
    failures.push(
      `npm ${name}@${metadata.version ?? "?"}: ${metadata.license ?? "UNKNOWN"}`,
    );
}

const cargo = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], {
    encoding: "utf8",
  }),
);
for (const pkg of cargo.packages) {
  if (!pkg.source) continue;
  if (!accepted(pkg.license))
    failures.push(
      `cargo ${pkg.name}@${pkg.version}: ${pkg.license ?? "UNKNOWN"}`,
    );
}

const fontRoot = resolve(root, "packages/viewer/fonts");
const fontManifest = JSON.parse(
  readFileSync(resolve(fontRoot, "manifest.json"), "utf8"),
);
if (!accepted(fontManifest.license))
  failures.push(`fonts: ${fontManifest.license ?? "UNKNOWN"}`);
for (const font of fontManifest.files ?? []) {
  const bytes = readFileSync(resolve(fontRoot, font.file));
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (bytes.byteLength !== font.bytes)
    failures.push(
      `font ${font.file}: expected ${font.bytes} bytes, got ${bytes.byteLength}`,
    );
  if (digest !== font.sha256)
    failures.push(`font ${font.file}: SHA-256 mismatch`);
}
for (const notice of ["OFL-1.1.txt", "THIRD_PARTY_NOTICES.md"])
  try {
    readFileSync(resolve(fontRoot, notice));
  } catch {
    failures.push(`font notice missing: ${notice}`);
  }

if (failures.length > 0) {
  console.error(
    "Dependency license policy failed:\n" +
      failures.map((failure) => `- ${failure}`).join("\n"),
  );
  process.exitCode = 1;
} else {
  console.log(
    `License policy passed for ${npmProduction.size} npm runtime packages, ${cargo.packages.length} Cargo packages, and ${fontManifest.files.length} verified font assets.`,
  );
}

function collectNpmProduction(dependencies) {
  for (const [name, metadata] of Object.entries(dependencies)) {
    if (metadata.version) npmProduction.add(`${name}@${metadata.version}`);
    collectNpmProduction(metadata.dependencies ?? {});
  }
}
