interface Request {
  readonly id: number;
  readonly type: "open" | "render" | "text" | "close";
  readonly data?: ArrayBuffer;
  readonly moduleUrl?: string;
  readonly pageIndex?: number;
  readonly dpi?: number;
}

interface PdfDocument {
  pageCount(): number;
  renderPagePng(pageIndex: number, dpi?: number): Uint8Array;
  pageTextJson(pageIndex: number): string;
  free(): void;
}

interface PdfModule {
  default(input?: string | URL): Promise<unknown>;
  PdfViewerDocument: new (data: Uint8Array) => PdfDocument;
}

const workerScope = self as unknown as DedicatedWorkerGlobalScope;
let document: PdfDocument | undefined;

workerScope.onmessage = async (event: MessageEvent<Request>) => {
  const request = event.data;
  try {
    if (request.type === "open") {
      if (!request.moduleUrl || !request.data)
        throw new Error("Invalid PDF open request");
      const module = (await import(request.moduleUrl)) as PdfModule;
      await module.default();
      document?.free();
      document = new module.PdfViewerDocument(new Uint8Array(request.data));
      workerScope.postMessage({
        id: request.id,
        ok: true,
        pageCount: document.pageCount(),
      });
    } else if (request.type === "render") {
      if (!document || request.pageIndex === undefined)
        throw new Error("PDF is not open");
      const png = document.renderPagePng(request.pageIndex, request.dpi);
      const data = png.slice().buffer;
      workerScope.postMessage({ id: request.id, ok: true, data }, [data]);
    } else if (request.type === "text") {
      if (!document || request.pageIndex === undefined)
        throw new Error("PDF is not open");
      workerScope.postMessage({
        id: request.id,
        ok: true,
        json: document.pageTextJson(request.pageIndex),
      });
    } else {
      document?.free();
      document = undefined;
      workerScope.postMessage({ id: request.id, ok: true });
    }
  } catch (error) {
    workerScope.postMessage({
      id: request.id,
      ok: false,
      message: error instanceof Error ? error.message : String(error),
    });
  }
};
