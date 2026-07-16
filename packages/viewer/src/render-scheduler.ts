import { abortError } from "./errors.js";

export type RenderPriority = "visible" | "adjacent" | "background";

interface QueuedRender<T> {
  readonly priority: number;
  readonly sequence: number;
  readonly signal: AbortSignal;
  readonly task: () => Promise<T>;
  readonly resolve: (value: T | PromiseLike<T>) => void;
  readonly reject: (reason?: unknown) => void;
}

const priorities: Readonly<Record<RenderPriority, number>> = {
  visible: 0,
  adjacent: 1,
  background: 2,
};

export class RenderScheduler {
  readonly #concurrency: number;
  readonly #queue: QueuedRender<unknown>[] = [];
  #active = 0;
  #sequence = 0;

  constructor(concurrency: number) {
    this.#concurrency = Math.max(1, Math.trunc(concurrency));
  }

  get active(): number {
    return this.#active;
  }

  get queued(): number {
    return this.#queue.length;
  }

  run<T>(
    priority: RenderPriority,
    signal: AbortSignal,
    task: () => Promise<T>,
  ): Promise<T> {
    if (signal.aborted) return Promise.reject(abortError());
    return new Promise<T>((resolve, reject) => {
      const queued: QueuedRender<T> = {
        priority: priorities[priority],
        sequence: this.#sequence++,
        signal,
        task,
        resolve,
        reject,
      };
      this.#queue.push(queued as QueuedRender<unknown>);
      this.#queue.sort(
        (left, right) =>
          left.priority - right.priority || left.sequence - right.sequence,
      );
      const onAbort = (): void => {
        const index = this.#queue.indexOf(queued as QueuedRender<unknown>);
        if (index < 0) return;
        this.#queue.splice(index, 1);
        reject(abortError());
      };
      signal.addEventListener("abort", onAbort, { once: true });
      void this.#drain();
    });
  }

  async #drain(): Promise<void> {
    while (this.#active < this.#concurrency && this.#queue.length > 0) {
      const queued = this.#queue.shift()!;
      if (queued.signal.aborted) {
        queued.reject(abortError());
        continue;
      }
      this.#active += 1;
      void queued
        .task()
        .then(queued.resolve, queued.reject)
        .finally(() => {
          this.#active -= 1;
          void this.#drain();
        });
    }
  }
}
