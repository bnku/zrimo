import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { expect, test } from "@playwright/test";

test("meets first-page and interaction responsiveness budgets", async ({
  page,
}) => {
  await page.goto("/");
  const metrics = await page.evaluate(async () => {
    const { ViewerClient } = await import("/main.js");
    const longTasks: number[] = [];
    let interactionStartedAt = Number.POSITIVE_INFINITY;
    const observer =
      typeof PerformanceObserver === "undefined"
        ? null
        : new PerformanceObserver((list) => {
            for (const entry of list.getEntries()) {
              if (entry.startTime >= interactionStartedAt)
                longTasks.push(entry.duration);
            }
          });
    try {
      observer?.observe({ type: "longtask", buffered: true });
    } catch {
      // The Long Tasks API is optional; the capability is reported below.
    }
    let firstRender = 0;
    const adapter = {
      id: "performance",
      formats: ["pdf"],
      open: async () => ({}),
      getInfo: async () => ({ format: "pdf", unit: "page", pageCount: 1_000 }),
      render: async (_handle: unknown, canvas: HTMLCanvasElement) => {
        canvas.width = 408;
        canvas.height = 528;
        canvas.getContext("2d")!.fillRect(0, 0, 4, 4);
        firstRender ||= performance.now();
      },
      getTextMap: async (_handle: unknown, pageIndex: number) => [
        {
          text: `page ${pageIndex}`,
          x: 10,
          y: 10,
          width: 80,
          height: 16,
        },
      ],
      close: () => {},
    };
    const host = document.createElement("div");
    Object.assign(host.style, { width: "900px", height: "650px" });
    document.body.append(host);
    const source = new Uint8Array(10 * 1024 * 1024);
    source.set(new TextEncoder().encode("%PDF-1.7\n"));
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container: host, overscan: 1 });
    const started = performance.now();
    await viewer.load(source, { fileName: "benchmark-10mib.pdf" });
    while (!firstRender)
      await new Promise((resolve) =>
        requestAnimationFrame(() => resolve(null)),
      );
    const firstVisibleMs = firstRender - started;
    interactionStartedAt = performance.now();
    for (let index = 0; index < 40; index += 1) {
      viewer.panBy(0, index % 2 === 0 ? 80 : -80);
      if (index % 4 === 0) viewer.setZoom(1 + (index % 8) * 0.05);
      await new Promise((resolve) =>
        requestAnimationFrame(() => resolve(null)),
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
    observer?.disconnect();
    const domUnits = host.querySelectorAll("canvas").length;
    await viewer.destroy();
    await client.destroy();
    host.remove();
    return {
      firstVisibleMs,
      maxLongTaskMs: Math.max(0, ...longTasks),
      longTaskApi: longTasks.length > 0 || "PerformanceObserver" in globalThis,
      domUnits,
    };
  });

  expect(metrics.firstVisibleMs).toBeLessThanOrEqual(2_500);
  expect(metrics.maxLongTaskMs).toBeLessThanOrEqual(50);
  expect(metrics.domUnits).toBeLessThanOrEqual(5);
  await mkdir(resolve("artifacts"), { recursive: true });
  await writeFile(
    resolve("artifacts/performance-chromium.json"),
    `${JSON.stringify(
      {
        schemaVersion: 1,
        host: "local Playwright Chromium; see docs/performance.md",
        fixtureBytes: 10 * 1024 * 1024,
        thresholds: { firstVisibleMs: 2_500, maxLongTaskMs: 50 },
        ...metrics,
      },
      null,
      2,
    )}\n`,
  );
});

test("repeated cleanup and operation timeout release resources", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const { ViewerClient, WorkerRpcClient } = await import("/main.js");
    let closes = 0;
    const adapter = {
      id: "cleanup",
      formats: ["pdf"],
      open: async () => ({}),
      getInfo: async () => ({ format: "pdf", unit: "page", pageCount: 1 }),
      render: async (_handle: unknown, canvas: HTMLCanvasElement) => {
        canvas.width = 32;
        canvas.height = 32;
      },
      close: () => {
        closes += 1;
      },
    };
    const host = document.createElement("div");
    document.body.append(host);
    const client = ViewerClient.create({ adapters: [adapter] });
    const viewer = client.createViewer({ container: host });
    for (let index = 0; index < 12; index += 1) {
      await viewer.load(new TextEncoder().encode("%PDF-1.7"), {
        fileName: `${index}.pdf`,
      });
      await viewer.close();
    }
    await viewer.destroy();
    await client.destroy();

    const workerUrl = URL.createObjectURL(
      new Blob(["self.onmessage = () => {}"], { type: "text/javascript" }),
    );
    const worker = new Worker(workerUrl);
    let terminations = 0;
    const originalTerminate = worker.terminate.bind(worker);
    worker.terminate = () => {
      terminations += 1;
      originalTerminate();
    };
    const rpc = new WorkerRpcClient(worker);
    let timeoutCode = "";
    try {
      await rpc.request("open", undefined, { timeoutMs: 20 });
    } catch (error) {
      timeoutCode = (error as { code?: string }).code ?? "";
    }
    URL.revokeObjectURL(workerUrl);
    const remainingChildren = host.childElementCount;
    host.remove();
    return { closes, terminations, timeoutCode, remainingChildren };
  });
  expect(result).toEqual({
    closes: 12,
    terminations: 1,
    timeoutCode: "resource-limit",
    remainingChildren: 0,
  });
});
