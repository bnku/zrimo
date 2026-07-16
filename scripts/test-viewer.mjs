import { execFileSync } from "node:child_process";
import { readdirSync } from "node:fs";

const node = process.execPath;
execFileSync(
  node,
  ["../../node_modules/typescript/bin/tsc", "-p", "tsconfig.test.json"],
  {
    stdio: "inherit",
  },
);
const tests = readdirSync(".test-dist/test")
  .filter((file) => file.endsWith(".test.js"))
  .map((file) => `.test-dist/test/${file}`);
execFileSync(node, ["--test", ...tests], {
  stdio: "inherit",
});
