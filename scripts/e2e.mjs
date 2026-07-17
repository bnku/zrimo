import { execFileSync } from "node:child_process";

await import("./fetch-corpus.mjs");
await import("./build-wasm.mjs");
execFileSync("npm", ["run", "build", "--workspace", "@zrimo/viewer"], {
  stdio: "inherit",
});
// Playwright reuses an already running local example server. Refresh its
// static directory first so a long-lived server cannot serve stale WASM or
// viewer assets from a previous build.
execFileSync("npm", ["run", "build", "--workspace", "@zrimo/example-vanilla"], {
  stdio: "inherit",
});
execFileSync(
  "node",
  ["node_modules/@playwright/test/cli.js", "test", ...process.argv.slice(2)],
  {
    stdio: "inherit",
  },
);
