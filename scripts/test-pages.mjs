import { execFileSync } from "node:child_process";

const explicitBase = process.env.PAGES_BASE;
const variants = explicitBase
  ? [{ base: explicitBase, port: 4184 }]
  : [
      { base: "/", port: 4184 },
      { base: "/zrimo/", port: 4185 },
    ];

for (const variant of variants) {
  const env = {
    ...process.env,
    CI: "1",
    PAGES_BASE: variant.base,
    PAGES_PORT: String(variant.port),
  };
  execFileSync(process.execPath, ["scripts/build-pages.mjs"], {
    env,
    stdio: "inherit",
  });
  execFileSync(
    process.execPath,
    [
      "node_modules/@playwright/test/cli.js",
      "test",
      "--config=playwright.pages.config.ts",
    ],
    { env, stdio: "inherit" },
  );
}
