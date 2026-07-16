import { StrictMode, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { ViewerClient, type ViewerApi } from "@docs-viewer-wasm/viewer";
import "@docs-viewer-wasm/viewer/styles.css";

function App() {
  const host = useRef<HTMLDivElement>(null);
  const viewer = useRef<ViewerApi>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!host.current) return;
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const instance = client.createViewer({
      container: host.current,
      ui: true,
      fit: "width",
    });
    viewer.current = instance;
    return () => {
      viewer.current = null;
      void instance.destroy().finally(() => client.destroy());
    };
  }, []);

  const open = async (file?: File) => {
    if (!file || !viewer.current) return;
    setError("");
    try {
      await viewer.current.load(file, { fileName: file.name });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  return (
    <main>
      <input
        aria-label="Open a document"
        type="file"
        onChange={(event) => void open(event.currentTarget.files?.[0])}
      />
      {error && <p role="alert">{error}</p>}
      <div ref={host} style={{ height: "calc(100vh - 48px)" }} />
    </main>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
