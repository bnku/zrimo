import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { expect, test } from "@playwright/test";

test.describe.configure({ mode: "serial" });

const cases = [
  { family: "modern-office", fileName: "sample.docx", threshold: 0.94 },
  { family: "pdf", fileName: "hello.pdf", threshold: 0.97 },
  { family: "image", fileName: "pixel.png", threshold: 0.97 },
] as const;
const results: Array<{
  family: string;
  fileName: string;
  threshold: number;
  ssim: number;
}> = [];

for (const fixture of cases) {
  test(`SSIM gate: ${fixture.family}`, async ({ page }, testInfo) => {
    await page.goto("/");
    await page.evaluate(async ({ fileName }) => {
      const { ViewerClient } = await import("/main.js");
      let source: Uint8Array;
      if (fileName === "pixel.png") {
        const sourceCanvas = document.createElement("canvas");
        sourceCanvas.width = 16;
        sourceCanvas.height = 16;
        const sourceContext = sourceCanvas.getContext("2d")!;
        sourceContext.fillStyle = "#3264c8";
        sourceContext.fillRect(0, 0, 16, 16);
        const blob = await new Promise<Blob>((resolve) =>
          sourceCanvas.toBlob((value) => resolve(value!), "image/png"),
        );
        source = new Uint8Array(await blob.arrayBuffer());
      } else {
        source = new Uint8Array(
          await (await fetch(`/corpus/${fileName}`)).arrayBuffer(),
        );
      }
      const canvas = document.createElement("canvas");
      canvas.dataset.testid = "fidelity-canvas";
      canvas.style.background = "white";
      document.body.replaceChildren(canvas);
      const client = ViewerClient.create({
        assetBaseUrl: new URL("/", location.href),
        fontPolicy: { mode: "offline" },
      });
      const viewer = client.createViewer();
      await viewer.load(source, { fileName });
      await viewer.renderPage(0, canvas, {
        zoom: 1,
        devicePixelRatio: 1,
        width: 816,
        height: 1056,
      });
      (
        window as Window & { __fidelityCleanup?: () => Promise<void> }
      ).__fidelityCleanup = async () => {
        await viewer.destroy();
        await client.destroy();
      };
    }, fixture);

    const canvas = page.getByTestId("fidelity-canvas");
    const snapshotName = `${fixture.family}.png`;
    await expect(canvas).toHaveScreenshot(snapshotName, {
      maxDiffPixelRatio: 1,
    });
    const [current, baseline] = await Promise.all([
      canvas.screenshot(),
      readFile(testInfo.snapshotPath(snapshotName)),
    ]);
    const ssim = await page.evaluate(
      async ({ currentBase64, baselineBase64 }) => {
        const decode = async (base64: string) => {
          const bytes = Uint8Array.from(atob(base64), (char) =>
            char.charCodeAt(0),
          );
          const bitmap = await createImageBitmap(new Blob([bytes]));
          const canvas = document.createElement("canvas");
          canvas.width = bitmap.width;
          canvas.height = bitmap.height;
          const context = canvas.getContext("2d")!;
          context.drawImage(bitmap, 0, 0);
          bitmap.close();
          return context.getImageData(0, 0, canvas.width, canvas.height);
        };
        const [left, right] = await Promise.all([
          decode(currentBase64),
          decode(baselineBase64),
        ]);
        if (left.width !== right.width || left.height !== right.height)
          return 0;
        const count = left.width * left.height;
        let leftMean = 0;
        let rightMean = 0;
        const luminance = (data: Uint8ClampedArray, offset: number) =>
          0.2126 * data[offset]! +
          0.7152 * data[offset + 1]! +
          0.0722 * data[offset + 2]!;
        for (let pixel = 0; pixel < count; pixel += 1) {
          leftMean += luminance(left.data, pixel * 4);
          rightMean += luminance(right.data, pixel * 4);
        }
        leftMean /= count;
        rightMean /= count;
        let leftVariance = 0;
        let rightVariance = 0;
        let covariance = 0;
        for (let pixel = 0; pixel < count; pixel += 1) {
          const leftDelta = luminance(left.data, pixel * 4) - leftMean;
          const rightDelta = luminance(right.data, pixel * 4) - rightMean;
          leftVariance += leftDelta * leftDelta;
          rightVariance += rightDelta * rightDelta;
          covariance += leftDelta * rightDelta;
        }
        const denominator = Math.max(1, count - 1);
        leftVariance /= denominator;
        rightVariance /= denominator;
        covariance /= denominator;
        const c1 = (0.01 * 255) ** 2;
        const c2 = (0.03 * 255) ** 2;
        return (
          ((2 * leftMean * rightMean + c1) * (2 * covariance + c2)) /
          ((leftMean ** 2 + rightMean ** 2 + c1) *
            (leftVariance + rightVariance + c2))
        );
      },
      {
        currentBase64: current.toString("base64"),
        baselineBase64: baseline.toString("base64"),
      },
    );
    expect(ssim).toBeGreaterThanOrEqual(fixture.threshold);
    results.push({ ...fixture, ssim });
    await page.evaluate(() =>
      (
        window as Window & { __fidelityCleanup?: () => Promise<void> }
      ).__fidelityCleanup?.(),
    );
  });
}

test.afterAll(async () => {
  await mkdir(resolve("artifacts"), { recursive: true });
  await writeFile(
    resolve("artifacts/fidelity-report.json"),
    `${JSON.stringify(
      { schemaVersion: 1, metric: "global-SSIM-luminance", results },
      null,
      2,
    )}\n`,
  );
});
