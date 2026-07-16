interface ConvertRequest {
  readonly data: ArrayBuffer;
  readonly format: "doc" | "xls" | "ppt";
  readonly moduleUrl: string;
}

interface LegacyModule {
  default(input?: string | URL): Promise<unknown>;
  convertLegacyToOoxml(data: Uint8Array, format: string): Uint8Array;
}

const workerScope = self as unknown as DedicatedWorkerGlobalScope;

workerScope.onmessage = async (event: MessageEvent<ConvertRequest>) => {
  try {
    const module = (await import(event.data.moduleUrl)) as LegacyModule;
    await module.default();
    const output = module.convertLegacyToOoxml(
      new Uint8Array(event.data.data),
      event.data.format,
    );
    const data = output.slice().buffer;
    workerScope.postMessage({ ok: true, data }, [data]);
  } catch (error) {
    workerScope.postMessage({
      ok: false,
      data: new ArrayBuffer(0),
      message: error instanceof Error ? error.message : String(error),
    });
  }
};
