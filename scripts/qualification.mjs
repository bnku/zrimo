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
    "converts_word97_xls_and_ppt_to_valid_ooxml_bytes",
    "--",
    "--ignored",
    "--exact",
  ],
  { env: environment, stdio: "inherit" },
);

if (process.env.ZRIMO_XLS_ORACLE) {
  execFileSync(
    "cargo",
    [
      "test",
      "-p",
      "legacy-office-wasm",
      "--test",
      "qualification",
      "local_xls_formatting_oracle_preserves_source_structure",
      "--",
      "--ignored",
      "--exact",
    ],
    { env: environment, stdio: "inherit" },
  );
}

execFileSync(
  "cargo",
  ["test", "-p", "legacy-doc", "--test", "qualification", "--", "--ignored"],
  { env: environment, stdio: "inherit" },
);

// PDF display qualification is browser-only now that PDF.js replaced the old
// Rust renderer. Its font/CMap corpus runs in tests/e2e/pdf.spec.ts.
