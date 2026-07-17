import { expect, test } from "@playwright/test";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

test("landing, documentation, and React demo ship as one Pages site", async ({
  page,
}) => {
  const failures: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      const location = message.location();
      failures.push(
        `${message.text()}${location.url ? ` (${location.url}:${location.lineNumber})` : ""}`,
      );
    }
  });
  page.on("pageerror", (error) => failures.push(error.message));
  page.on("response", (response) => {
    if (response.status() >= 400) {
      failures.push(`${response.status()} ${response.url()}`);
    }
  });

  await page.goto(".");
  await expect(page).toHaveTitle(/Zrimo/);
  await expect(
    page.getByRole("heading", { name: /Any document/i }),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: /Open React demo/i }),
  ).toBeVisible();
  await expect(
    page.getByText(/release candidate|release checklist|release checks/i),
  ).toHaveCount(0);
  expect(
    existsSync(
      resolve(
        import.meta.dirname,
        "../../docs/.vitepress/dist/release-checklist.html",
      ),
    ),
  ).toBe(false);
  const featureIcons = page.locator(".VPFeature img");
  await expect(featureIcons).toHaveCount(6);
  await expect(page.locator("a.VPFeature.VPLink")).toHaveCount(6);
  expect(
    await featureIcons.evaluateAll((icons) =>
      icons.every(
        (icon) =>
          icon instanceof HTMLImageElement &&
          icon.complete &&
          icon.naturalWidth > 0,
      ),
    ),
  ).toBe(true);

  await page.locator('a[href$="/performance"]').first().click();
  await expect(
    page.getByRole("heading", { name: "Rendering and memory controls" }),
  ).toBeVisible();
  await expect(page.getByText(/ViewerClient\.create/)).toBeVisible();

  await page.goto("getting-started");
  await expect(
    page.getByRole("heading", { name: "Getting started" }),
  ).toBeVisible();
  await expect(page.getByText("npm install @zrimo/viewer")).toBeVisible();

  await page.goto("demo/");
  await expect(page).not.toHaveTitle(/404|Page not found/i);
  await expect(page.getByText("React integration playground")).toBeVisible();
  await expect(page.getByRole("tab", { name: /Built-in UI/i })).toBeVisible();
  await expect(
    page.getByRole("tab", { name: /React controls/i }),
  ).toBeVisible();
  await expect(page.getByRole("tab", { name: /Headless API/i })).toBeVisible();
  expect(failures).toEqual([]);
});
