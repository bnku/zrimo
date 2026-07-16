# Worker protocol

The protocol is an internal discriminated union defined in `worker-protocol.ts`. It is versioned with the package and is not a public compatibility surface.

## Main to worker

- `{ kind: "request", id, operation, payload? }`
- `{ kind: "cancel", id }`

Operations are `init`, `open`, `get-info`, `render`, `get-text-map`, `close`, and `destroy`. `open` transfers an `ArrayBuffer` plus detected format, resolved limits, and optional filename/MIME. `render` carries page index, zoom, and device-pixel ratio.

## Worker to main

- `{ kind: "progress", id, progress }`
- `{ kind: "warning", id, warning }`
- `{ kind: "success", id, result? }`
- `{ kind: "failure", id, error: ViewerErrorData }`

Array buffers and `ImageBitmap`s are transferables. The worker endpoint owns one `AbortController` per active request. Cancellation aborts that controller; late messages for already-settled IDs are ignored by the client.

`WorkerRpcClient` removes the request's abort listener on settlement. Worker `error` or `messageerror` rejects all pending requests as `worker-crashed`. `destroy()` removes all worker listeners, terminates the worker exactly once, and rejects remaining requests.

An adapter retains source bytes on the main side for download, so it transfers a dedicated copy to the worker. Future streaming support may transfer chunks, but it must preserve the same request/error semantics.

