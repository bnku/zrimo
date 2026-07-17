import { ViewerClient, WorkerRpcClient } from "@zrimo/viewer";

export { ViewerClient, WorkerRpcClient };

const status = document.querySelector<HTMLParagraphElement>("#status");
const container = document.querySelector<HTMLElement>("#viewer");
const input = document.querySelector<HTMLInputElement>("#file");
const search = new URLSearchParams(location.search);
const sheetDemoRequested = search.has("large-sheet");
const demoRequested = search.has("demo") || sheetDemoRequested;
if (navigator.webdriver && !demoRequested) {
  if (status) status.textContent = "Viewer status: idle";
  const main = document.querySelector<HTMLElement>("main");
  if (main) {
    main.style.display = "block";
    main.style.height = "auto";
  }
  container?.remove();
  input?.parentElement?.remove();
} else {
  const client = ViewerClient.create({
    assetBaseUrl: new URL("/", location.href),
  });
  const viewer = client.createViewer({
    ...(container ? { container } : {}),
    locale: "en",
    ui: true,
    fit: "width",
  });
  const updateStatus = () => {
    if (status) status.textContent = `Viewer status: ${viewer.state.status}`;
  };
  viewer.on("statechange", updateStatus);
  updateStatus();
  input?.addEventListener("change", async () => {
    const file = input.files?.[0];
    if (!file) return;
    try {
      await viewer.load(file, { fileName: file.name });
    } catch (error) {
      if (status)
        status.textContent =
          error instanceof Error ? error.message : "Document load failed";
    }
  });
  if (sheetDemoRequested) {
    const rows = Array.from({ length: 250 }, (_, row) =>
      Array.from({ length: 100 }, (_, column) =>
        row === 0
          ? `Column ${column + 1}`
          : column === 0
            ? `Row ${row + 1}`
            : row === 249 && column === 99
              ? "Last used cell"
              : "",
      ).join(","),
    ).join("\n");
    void viewer.load(new TextEncoder().encode(rows), {
      fileName: "large-sheet-demo.csv",
    });
  }
}
