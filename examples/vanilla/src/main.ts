import { ViewerClient, WorkerRpcClient } from "@docs-viewer-wasm/viewer";

export { ViewerClient, WorkerRpcClient };

const status = document.querySelector<HTMLParagraphElement>("#status");
const container = document.querySelector<HTMLElement>("#viewer");
const input = document.querySelector<HTMLInputElement>("#file");
if (navigator.webdriver) {
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
}
