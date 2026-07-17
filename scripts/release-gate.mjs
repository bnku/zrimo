import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const [status, regressions, packageManifest] = await Promise.all([
  readJson(resolve(root, "release-status.json")),
  readJson(resolve(root, "tests/regressions/manifest.json")),
  readJson(resolve(root, "packages/viewer/package.json")),
]);
const channel = readChannel(process.argv.slice(2));

if (
  status.schemaVersion !== 1 ||
  !Array.isArray(status.blockedChannels) ||
  !Array.isArray(regressions.cases) ||
  status.packageVersion !== packageManifest.version ||
  (status.publishedArtifact &&
    (status.publishedArtifact.version !== packageManifest.version ||
      !/^sha512-[A-Za-z0-9+/]+={0,2}$/.test(
        status.publishedArtifact.integrity,
      ) ||
      !/^[a-f0-9]{40}$/.test(status.publishedArtifact.shasum) ||
      !/^[a-f0-9]{64}$/.test(status.publishedArtifact.sha256)))
) {
  throw new Error("Release status is invalid; refusing promotion.");
}

if (
  status.state !== "ready" ||
  status.blockedChannels.includes(channel) ||
  regressions.cases.some((entry) => entry.status !== "pass")
) {
  throw new Error(
    `Release channel ${channel} is blocked by ${status.reason ?? "release policy"}; required corrective tasks: ${(status.requiredTasks ?? []).join(", ")}.`,
  );
}

async function readJson(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    throw new Error("Release status is invalid; refusing promotion.");
  }
}

console.log(`Release gate passed for ${channel}.`);

function readChannel(args) {
  const index = args.indexOf("--channel");
  const channel = index >= 0 ? args[index + 1] : undefined;
  if (!channel || !["alpha", "beta", "latest"].includes(channel)) {
    throw new Error(
      "Usage: npm run release:gate -- --channel <alpha|beta|latest>",
    );
  }
  return channel;
}
