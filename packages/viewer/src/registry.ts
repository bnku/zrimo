import type { DocumentAdapter, DocumentFormat } from "./contracts.js";
import { ViewerError } from "./errors.js";

export class AdapterRegistry {
  readonly #adapters = new Map<DocumentFormat, DocumentAdapter>();

  constructor(adapters: readonly DocumentAdapter[] = []) {
    for (const adapter of adapters) this.register(adapter);
  }

  register(adapter: DocumentAdapter): () => void {
    if (!adapter.id.trim())
      throw new ViewerError("internal", "Adapter id must not be empty");
    for (const format of adapter.formats) {
      if (this.#adapters.has(format))
        throw new ViewerError(
          "internal",
          `An adapter is already registered for ${format}`,
        );
      this.#adapters.set(format, adapter);
    }
    return () => {
      for (const format of adapter.formats)
        if (this.#adapters.get(format) === adapter)
          this.#adapters.delete(format);
    };
  }

  resolve(format: DocumentFormat): DocumentAdapter {
    const adapter = this.#adapters.get(format);
    if (!adapter)
      throw new ViewerError(
        "unsupported-format",
        `No adapter is registered for ${format}`,
        {
          details: { format },
        },
      );
    return adapter;
  }

  async destroy(): Promise<void> {
    const unique = new Set(this.#adapters.values());
    this.#adapters.clear();
    await Promise.all([...unique].map(async (adapter) => adapter.destroy?.()));
  }
}
