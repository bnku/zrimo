interface Request {
  readonly id: number;
  readonly type: "open" | "render" | "close";
  readonly data?: ArrayBuffer;
  readonly moduleUrl?: string;
  readonly maxPixels?: number;
  readonly pageIndex?: number;
}

interface TiffDocument {
  pageCount(): number;
  pageWidth(pageIndex: number): number;
  pageHeight(pageIndex: number): number;
  renderPagePng(pageIndex: number): Uint8Array;
  free(): void;
}

interface ImageModule {
  default(input?: string | URL): Promise<unknown>;
  TiffViewerDocument: new (data: Uint8Array, maxPixels: number) => TiffDocument;
}

const workerScope = self as unknown as DedicatedWorkerGlobalScope;
let document: TiffDocument | undefined;

workerScope.onmessage = async (event: MessageEvent<Request>) => {
  const request = event.data;
  try {
    if (request.type === "open") {
      if (!request.moduleUrl || !request.data || !request.maxPixels)
        throw new Error("Invalid TIFF open request");
      const module = (await import(request.moduleUrl)) as ImageModule;
      await module.default();
      document?.free();
      document = new module.TiffViewerDocument(
        new Uint8Array(request.data),
        request.maxPixels,
      );
      const pages = Array.from(
        { length: document.pageCount() },
        (_, pageIndex) => ({
          width: document!.pageWidth(pageIndex),
          height: document!.pageHeight(pageIndex),
        }),
      );
      workerScope.postMessage({ id: request.id, ok: true, pages });
    } else if (request.type === "render") {
      if (!document || request.pageIndex === undefined)
        throw new Error("TIFF is not open");
      const png = document.renderPagePng(request.pageIndex);
      const data = png.slice().buffer;
      workerScope.postMessage({ id: request.id, ok: true, data }, [data]);
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
