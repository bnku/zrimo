# Задача 02. Runtime, worker protocol и безопасность

**Статус:** ✅ Выполнена 2026-07-16

## Цель

Реализовать общий browser runtime, который определяет формат, загружает нужный adapter, изолирует parsing в worker, управляет жизненным циклом ресурсов и одинаково сообщает progress, warnings и errors независимо от backend.

## Источники требований

- [`../00-roadmap.md`](../00-roadmap.md)
- [`./01-foundation-dependency-qualification.md`](./01-foundation-dependency-qualification.md)

## Зависимости

- Задача 01 завершена; версии backends и adapter contract зафиксированы.

## Что входит в задачу

- Internal `FormatAdapter`/`DocumentBackend` contracts и registry.
- Magic-byte/content-type detection с filename hint только как дополнительным сигналом.
- ESM Web Worker protocol с request IDs, progress, cancellation и transferables.
- Resource limits, typed errors, warning model и deterministic cleanup.
- Skeleton `ViewerClient`/`DocumentViewer` lifecycle без окончательного viewport UI.
- Source loading для URL, Blob/File, ArrayBuffer и Uint8Array.

## Что НЕ делаем в этой задаче

- Не реализуем format-specific layout/rendering.
- Не строим toolbar, thumbnails или search UI.
- Не добавляем true streaming/range parsing как обязательную возможность v1.
- Не выполняем external relationships, scripts, macros или embedded executables.

## Основные задачи по слоям

### Feature

- Определить adapter lifecycle: `open`, `getInfo`, `render`, `getTextMap`, `close`, `destroy`.
- Реализовать format detector для всей согласованной matrix и типизированный `unsupported-format`.
- Реализовать worker messages для init/open/progress/render/cancel/close/destroy; большие buffers передавать как transferables.
- Реализовать configurable limits с defaults: 100 MiB input, 512 MiB expanded Office data, 64 MiB ZIP entry.
- Добавить `AbortSignal` на fetch, parsing, conversion и rendering queue.
- Ввести `ViewerError` codes и serializable worker error envelope без проверки строковых сообщений.
- Хранить original source bytes/Blob только до `close`/`destroy`, чтобы basic UI мог скачать исходник.
- Реализовать cleanup object URLs, ImageBitmap, worker listeners, WASM handles и DOM hooks; worker termination должен освобождать WASM memory.
- Добавить hooks `fetch`, `logger`, `assetBaseUrl` и запретить скрытые сетевые запросы.

### Tests

- Unit tests format signatures, mismatched extensions, MIME hints и unsupported inputs.
- Worker contract tests для success, progress, cancellation, timeout-like abort, backend error и worker crash.
- Resource-limit tests для oversized input, ZIP bomb, oversized entry и excessive decoded pixels.
- Lifecycle tests load→close→load, concurrent load cancellation, repeated destroy и use-after-destroy errors.
- Проверить, что source URL использует custom fetch и CORS/network errors получают стабильный code.
- Memory smoke: после `destroy()` worker отсутствует, object URLs revoked, listeners и bitmap handles освобождены.

### Docs

- Создать `docs/api/runtime.md` с lifecycle и source-loading contract.
- Создать `docs/security.md` с threat model, default limits, blocked active content и host responsibilities.
- Зафиксировать worker protocol и error catalog в internal design documentation.

## Критерии готовности

- Любой source проходит единый detect/open/cancel/close pipeline.
- Backend-specific panic/error не рушит main thread и превращается в typed `ViewerError`.
- Limits применяются до опасного allocation или archive expansion.
- `destroy()` идемпотентен и освобождает все runtime resources.
- Runtime не выполняет незаявленных network calls.
- Format adapters из следующих задач можно подключать без изменения публичного lifecycle.

## Фактический результат

### Feature

- Реализованы SSR-safe `ViewerClient`, `DocumentViewer`, `AdapterRegistry` и lifecycle `load/open → close → destroy` с сохранением original bytes только на время открытого документа.
- Content-first detector покрывает полную v1 matrix: signatures raster/PDF, OOXML central-directory names, OLE stream names, SVG и hint-only CSV/TSV; несовпадения дают typed warning.
- Source loader поддерживает URL/custom fetch, Blob/File, ArrayBuffer и Uint8Array; размер проверяется по Blob/Content-Length и во время stream read.
- Добавлены ESM worker RPC client/endpoint, request IDs, progress, warnings, transferables, cancel, crash handling и готовый `WorkerDocumentAdapter`.
- Реализованы defaults 100 MiB input / 512 MiB expanded Office / 64 MiB entry / 100 MP raster и проверки ZIP central directory и PNG/GIF/JPEG/WebP/BMP/TIFF headers.

### Tests

- 22 TypeScript unit tests закрывают signatures/hints, ZIP/pixel limits, custom fetch/network errors, lifecycle/reload, concurrent-load race, cancellation, repeated destroy и worker success/progress/warning/backend error/cancel/crash.
- В ходе тестирования найдена и исправлена race condition регистрации `AbortController` между конкурентными `load()`.
- Полный `npm run check` и существующие четыре Chromium qualification smoke проходят; license inventory остаётся зелёным.

### Docs

- Lifecycle и source contract: [`../../api/runtime.md`](../../api/runtime.md).
- Threat model и limits: [`../../security.md`](../../security.md).
- Worker envelopes и ownership: [`../../internal/worker-protocol.md`](../../internal/worker-protocol.md).
