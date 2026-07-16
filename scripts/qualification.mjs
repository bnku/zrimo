import { execFileSync } from "node:child_process";

import { corpusDirectory } from "./fetch-corpus.mjs";

const environment = { ...process.env, CORPUS_DIR: corpusDirectory };
for (const packageName of ["legacy-office-wasm", "pdf-wasm"]) {
  execFileSync(
    "cargo",
    ["test", "-p", packageName, "--test", "qualification", "--", "--ignored"],
    { env: environment, stdio: "inherit" },
  );
}
