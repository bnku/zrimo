import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const manifest = JSON.parse(
  await readFile(resolve(root, "packages/viewer/package.json"), "utf8"),
);
const expectedTag = `v${manifest.version}`;
const requestedTag = valueAfter("--tag") ?? process.env.GITHUB_REF_NAME;

if (!/^\d+\.\d+\.\d+$/.test(manifest.version)) {
  throw new Error(
    `Refusing a stable release for prerelease version ${manifest.version}.`,
  );
}
if (requestedTag && requestedTag !== expectedTag) {
  throw new Error(
    `Release tag ${requestedTag} must match package version ${expectedTag}.`,
  );
}

for (const [command, args] of [
  ["npm", ["run", "check"]],
  ["npm", ["run", "test:qualification"]],
  ["npm", ["run", "test:e2e"]],
  ["npm", ["run", "test:e2e:matrix"]],
  ["npm", ["run", "test:pages"]],
  ["npm", ["run", "fuzz:js"]],
  ["npm", ["run", "audit:vulnerabilities"]],
  ["npm", ["run", "report:size"]],
  ["npm", ["run", "report:sbom"]],
  ["npm", ["run", "test:pack"]],
  ["npm", ["run", "release:gate", "--", "--channel", "latest"]],
]) {
  execFileSync(command, args, { cwd: root, stdio: "inherit" });
}

console.log(
  `Release verification passed for ${expectedTag}. No publication was performed.`,
);

function valueAfter(flag) {
  const index = process.argv.indexOf(flag);
  return index < 0 ? undefined : process.argv[index + 1];
}
