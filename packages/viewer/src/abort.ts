import { abortError } from "./errors.js";

export function linkedAbortController(
  ...signals: readonly (AbortSignal | undefined)[]
): AbortController {
  const controller = new AbortController();
  const abort = () => controller.abort(abortError());
  for (const signal of signals) {
    if (!signal) continue;
    if (signal.aborted) {
      abort();
      break;
    }
    signal.addEventListener("abort", abort, {
      once: true,
      signal: controller.signal,
    });
  }
  return controller;
}
