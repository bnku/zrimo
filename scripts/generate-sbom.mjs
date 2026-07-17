import { execFileSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const npmTree = JSON.parse(
  execFileSync("npm", ["ls", "--all", "--omit=dev", "--json"], {
    cwd: root,
    encoding: "utf8",
  }),
);
const cargo = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], {
    cwd: root,
    encoding: "utf8",
  }),
);
const packages = [];
const seen = new Set();
collectNpm(npmTree.dependencies ?? {});
for (const pkg of cargo.packages) {
  if (!pkg.source) continue;
  const key = `cargo:${pkg.name}@${pkg.version}`;
  if (seen.has(key)) continue;
  seen.add(key);
  packages.push({
    SPDXID: spdxId(key),
    name: pkg.name,
    versionInfo: pkg.version,
    downloadLocation: pkg.source,
    licenseConcluded: pkg.license ?? "NOASSERTION",
    licenseDeclared: pkg.license ?? "NOASSERTION",
    supplier: "NOASSERTION",
  });
}
const document = {
  spdxVersion: "SPDX-2.3",
  dataLicense: "CC0-1.0",
  SPDXID: "SPDXRef-DOCUMENT",
  name: "zrimo-release-sbom",
  documentNamespace: `https://zrimo.dev/spdx/${Date.now()}`,
  creationInfo: {
    created: new Date().toISOString(),
    creators: ["Tool: zrimo/scripts/generate-sbom.mjs"],
  },
  packages,
};
await mkdir(resolve(root, "artifacts"), { recursive: true });
await writeFile(
  resolve(root, "artifacts/sbom.spdx.json"),
  `${JSON.stringify(document, null, 2)}\n`,
);
console.log(`SPDX SBOM generated for ${packages.length} third-party packages.`);

function collectNpm(dependencies) {
  for (const [name, metadata] of Object.entries(dependencies)) {
    const key = `npm:${name}@${metadata.version ?? "unknown"}`;
    if (!seen.has(key)) {
      seen.add(key);
      packages.push({
        SPDXID: spdxId(key),
        name,
        versionInfo: metadata.version ?? "unknown",
        downloadLocation: `https://registry.npmjs.org/${name}`,
        licenseConcluded: "NOASSERTION",
        licenseDeclared: "NOASSERTION",
        supplier: "NOASSERTION",
      });
    }
    collectNpm(metadata.dependencies ?? {});
  }
}

function spdxId(value) {
  return `SPDXRef-${value.replaceAll(/[^A-Za-z0-9.-]/g, "-")}`;
}
