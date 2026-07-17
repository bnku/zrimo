#!/usr/bin/env node

import { cp, mkdir } from "node:fs/promises";
import { resolve } from "node:path";

const packageRoot = resolve(import.meta.dirname, "..");
const destination = process.argv[2];

if (!destination || destination === "--help" || destination === "-h") {
  console.log("Usage: zrimo-copy-assets <public-directory>");
  console.log("Copies Zrimo workers, WASM/PDF assets, and optional fonts.");
  process.exit(destination ? 0 : 1);
}

const output = resolve(process.cwd(), destination);
await mkdir(output, { recursive: true });
for (const directory of ["assets", "fonts", "workers"]) {
  await cp(
    resolve(packageRoot, "dist", directory),
    resolve(output, directory),
    {
      recursive: true,
      force: true,
    },
  );
}
console.log(`Zrimo runtime assets copied to ${output}`);
