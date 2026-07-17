import { execFileSync } from "node:child_process";

import { corpusDirectory } from "./fetch-corpus.mjs";

const environment = { ...process.env, CORPUS_DIR: corpusDirectory };
execFileSync(
  "cargo",
  [
    "test",
    "-p",
    "legacy-office-wasm",
    "--test",
    "qualification",
    "--",
    "--ignored",
  ],
  { env: environment, stdio: "inherit" },
);

// PDF display qualification is browser-only now that PDF.js replaced the old
// Rust renderer. Its font/CMap corpus runs in tests/e2e/pdf.spec.ts.
