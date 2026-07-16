import { execFileSync } from "node:child_process";

await import("./fetch-corpus.mjs");
await import("./build-wasm.mjs");
execFileSync(
  "npm",
  ["run", "build", "--workspace", "@docs-viewer-wasm/viewer"],
  {
    stdio: "inherit",
  },
);
execFileSync(
  "node",
  ["node_modules/@playwright/test/cli.js", "test", ...process.argv.slice(2)],
  {
    stdio: "inherit",
  },
);
