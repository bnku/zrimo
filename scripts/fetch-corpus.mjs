import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const manifest = JSON.parse(
  await readFile(resolve(root, "tests/corpus/manifest.json"), "utf8"),
);
export const corpusDirectory = resolve(root, ".cache/corpus");

await mkdir(corpusDirectory, { recursive: true });

for (const file of manifest.files) {
  const destination = resolve(corpusDirectory, file.name);
  let bytes;
  try {
    bytes = await readFile(destination);
  } catch {
    bytes = undefined;
  }

  if (!bytes || sha256(bytes) !== file.sha256) {
    const url = `${file.sourceRepository}/raw/${file.sourceCommit}/${file.sourcePath}`;
    const response = await fetch(url);
    if (!response.ok)
      throw new Error(`Failed to fetch ${file.name}: HTTP ${response.status}`);
    bytes = Buffer.from(await response.arrayBuffer());
    if (sha256(bytes) !== file.sha256)
      throw new Error(`Checksum mismatch for ${file.name}`);
    await writeFile(destination, bytes);
  }
}

console.log(
  `Corpus ready: ${manifest.files.length} files in ${corpusDirectory}`,
);

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}
