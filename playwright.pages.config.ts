import { defineConfig, devices } from "@playwright/test";
import { existsSync } from "node:fs";

const chromiumPath =
  process.env.CHROMIUM_PATH ??
  (existsSync("/usr/bin/chromium") ? "/usr/bin/chromium" : undefined);
const port = Number(process.env.PAGES_PORT ?? 4174);

export default defineConfig({
  testDir: "./tests/pages",
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        launchOptions: chromiumPath ? { executablePath: chromiumPath } : {},
      },
    },
  ],
  use: {
    baseURL: `http://127.0.0.1:${port}${process.env.PAGES_BASE ?? "/"}`,
  },
  webServer: {
    command: `vitepress preview docs --host 127.0.0.1 --port ${port}`,
    port,
    reuseExistingServer: !process.env.CI,
  },
});
