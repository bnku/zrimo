import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type {
  WorkerInboundMessage,
  WorkerLike,
  WorkerOutboundMessage,
  WorkerScopeLike,
} from "../src/index.js";
import {
  attachWorkerEndpoint,
  WorkerRpcClient,
  ViewerError,
} from "../src/index.js";

describe("worker RPC contract", () => {
  it("routes progress and success by request id", async () => {
    const worker = fakeWorker();
    const progress: number[] = [];
    const client = new WorkerRpcClient(worker.api);
    const request = client.request<{ ok: boolean }>("open", undefined, {
      onProgress: (event) => progress.push(event.ratio ?? 0),
    });
    const id = worker.sent[0]!.id;
    worker.emit({
      kind: "progress",
      id,
      progress: { phase: "parsing", ratio: 0.5 },
    });
    worker.emit({ kind: "success", id, result: { ok: true } });
    assert.deepEqual(await request, { ok: true });
    assert.deepEqual(progress, [0.5]);
  });

  it("reconstructs a typed backend error", async () => {
    const worker = fakeWorker();
    const client = new WorkerRpcClient(worker.api);
    const request = client.request("render");
    worker.emit({
      kind: "failure",
      id: worker.sent[0]!.id,
      error: {
        name: "ViewerError",
        code: "render-failed",
        message: "bad page",
      },
    });
    await assert.rejects(request, isCode("render-failed"));
  });

  it("sends cancel and rejects when AbortSignal fires", async () => {
    const worker = fakeWorker();
    const client = new WorkerRpcClient(worker.api);
    const controller = new AbortController();
    const request = client.request("open", undefined, {
      signal: controller.signal,
    });
    controller.abort();
    await assert.rejects(request, isCode("aborted"));
    assert.equal(worker.sent.at(-1)?.kind, "cancel");
  });

  it("rejects pending requests and terminates after worker crash", async () => {
    const worker = fakeWorker();
    const client = new WorkerRpcClient(worker.api);
    const request = client.request("open");
    worker.crash();
    await assert.rejects(request, isCode("worker-crashed"));
    client.destroy();
    client.destroy();
    assert.equal(worker.terminations, 1);
  });

  it("terminates a worker operation that exceeds its time budget", async () => {
    const worker = fakeWorker();
    const client = new WorkerRpcClient(worker.api);
    const request = client.request("open", undefined, { timeoutMs: 5 });
    await assert.rejects(request, (error: unknown) => {
      assert.ok(error instanceof ViewerError);
      assert.equal(error.code, "resource-limit");
      assert.match(error.message, /maxOperationMs/);
      return true;
    });
    assert.equal(worker.sent.at(-1)?.kind, "cancel");
    assert.equal(worker.terminations, 1);
    await assert.rejects(client.request("get-info"), isCode("lifecycle-error"));
  });
});

describe("worker endpoint", () => {
  it("emits progress, warning, and a success envelope", async () => {
    const scope = fakeScope();
    attachWorkerEndpoint(scope.api, async (_operation, payload, context) => {
      context.reportProgress({ phase: "parsing", ratio: 0.5 });
      context.reportWarning({
        code: "fidelity-degraded",
        message: "qualification warning",
      });
      return payload;
    });
    scope.send({
      kind: "request",
      id: 7,
      operation: "open",
      payload: { ok: true },
    });
    await nextTask();
    assert.deepEqual(
      scope.outputs.map((message) => message.kind),
      ["progress", "warning", "success"],
    );
  });

  it("turns cancel into a typed aborted failure", async () => {
    const scope = fakeScope();
    attachWorkerEndpoint(
      scope.api,
      async (_operation, _payload, context) =>
        new Promise((_, reject) =>
          context.signal.addEventListener(
            "abort",
            () => reject(new DOMException("aborted", "AbortError")),
            { once: true },
          ),
        ),
    );
    scope.send({ kind: "request", id: 8, operation: "open" });
    scope.send({ kind: "cancel", id: 8 });
    await nextTask();
    const failure = scope.outputs.at(-1);
    assert.equal(failure?.kind, "failure");
    if (failure?.kind === "failure")
      assert.equal(failure.error.code, "aborted");
  });
});

function fakeWorker() {
  const messages = new Set<
    (event: MessageEvent<WorkerOutboundMessage>) => void
  >();
  const errors = new Set<(event: Event) => void>();
  const sent: Array<{ kind: string; id: number }> = [];
  let terminations = 0;
  const api = {
    postMessage(message: { kind: string; id: number }) {
      sent.push(message);
    },
    addEventListener(type: string, listener: never) {
      if (type === "message") messages.add(listener);
      else errors.add(listener);
    },
    removeEventListener(type: string, listener: never) {
      if (type === "message") messages.delete(listener);
      else errors.delete(listener);
    },
    terminate() {
      terminations += 1;
    },
  } as unknown as WorkerLike;
  return {
    api,
    sent,
    get terminations() {
      return terminations;
    },
    emit(message: WorkerOutboundMessage) {
      for (const listener of messages)
        listener({ data: message } as MessageEvent<WorkerOutboundMessage>);
    },
    crash() {
      for (const listener of errors) listener(new Event("error"));
    },
  };
}

function fakeScope() {
  let listener:
    ((event: MessageEvent<WorkerInboundMessage>) => void) | undefined;
  const outputs: WorkerOutboundMessage[] = [];
  const api = {
    postMessage(message: WorkerOutboundMessage) {
      outputs.push(message);
    },
    addEventListener(
      _type: "message",
      next: (event: MessageEvent<WorkerInboundMessage>) => void,
    ) {
      listener = next;
    },
    removeEventListener() {
      listener = undefined;
    },
  } satisfies WorkerScopeLike;
  return {
    api,
    outputs,
    send(message: WorkerInboundMessage) {
      listener?.({ data: message } as MessageEvent<WorkerInboundMessage>);
    },
  };
}

async function nextTask(): Promise<void> {
  await new Promise<void>((resolve) => setImmediate(resolve));
}

function isCode(code: string): (error: unknown) => boolean {
  return (error) => error instanceof ViewerError && error.code === code;
}
