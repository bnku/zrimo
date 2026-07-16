# Задача 05. Viewport, interaction и публичный API

**Статус:** ✅ Выполнена 2026-07-16

## Результат

Реализован единый `DocumentViewer` API и SSR-safe headless mode, continuous/single viewport с bounded virtualization, overscan, DPR/resize invalidation и отменой stale renders. Pan/zoom/fit/navigation поддерживают pointer, wheel, pinch и keyboard; zoom gestures используют быстрый bitmap preview и отложенный crisp render без смены логической позиции.

Добавлены Unicode NFKC/case-fold search с отображением исходных offsets, cross-page logical text selection, spreadsheet drag/Shift+arrow selection с merged ranges и TSV copy, immutable state/info/results, typed events и original-source download. Headless API покрывает page/thumbnail/sheet-region rendering и cancellation.

Проверки: 51 TypeScript test, Rust workspace tests/clippy, license gate и 9 Chromium E2E проходят; браузерный сценарий на 10 000 страниц подтверждает bounded DOM/canvas count. Public contract описан в `docs/api/reference.md`, `docs/api/headless.md`, `docs/architecture/viewport.md` и migration note.

## Цель

Построить единый виртуализированный viewport и стабильный TypeScript API управления документом поверх всех format backends. Пользователь должен одинаково управлять pan, zoom, navigation, search и selection независимо от исходного формата.

## Источники требований

- [`../00-roadmap.md`](../00-roadmap.md)
- [`./02-runtime-security.md`](./02-runtime-security.md)
- [`./03-office-pipeline.md`](./03-office-pipeline.md)
- [`./04-pdf-images-csv-svg.md`](./04-pdf-images-csv-svg.md)

## Зависимости

- Задачи 02–04 завершены; все backends реализуют общий render/text/cell contract.

## Что входит в задачу

- `ViewerClient`, `DocumentViewer` и public TypeScript types.
- Continuous/single paginated viewport и spreadsheet viewport.
- Virtualization, render scheduling, cache, resize/DPR handling.
- Pan, zoom, fit, page/slide/sheet navigation и thumbnails data API.
- Unicode search, text selection/copy и spreadsheet cell range selection/copy.
- Typed events, state snapshots и headless rendering methods.

## Что НЕ делаем в этой задаче

- Не реализуем visual toolbar и panels; они относятся к задаче 06.
- Не добавляем annotations, editing, print или collaboration.
- Не добавляем fuzzy/AI search; v1 использует literal normalized search.
- Не обещаем browser-native selection DOM для форматов без text map.

## Основные задачи по слоям

### Feature

- Реализовать `ViewerClient.create(options)` и `createViewer({ container?, ui? })`; import пакета не должен обращаться к DOM.
- Реализовать `DocumentViewer.load/close/destroy` и immutable `DocumentInfo`/capabilities/warnings.
- Построить virtual page list с overscan и recycling; spreadsheet использовать tile/viewport rendering, а не создавать DOM на каждую cell.
- Во время pan/pinch/wheel применять transform preview, после settle запрашивать crisp render на актуальном DPR.
- Реализовать zoom bounds, `setZoom`, `zoomIn/out`, `fitWidth`, `fitPage`, `panBy` и view state events.
- Реализовать `goToPage`, `next`, `previous`, `setSheet`, visible unit tracking и sheet/page change events.
- Построить text layer из backend text map с сохранением logical order для copy; mixed RTL/LTR ranges не должны разворачиваться визуальным порядком.
- Реализовать cell selection model, mouse drag, keyboard extension, TSV copy и programmatic `selectCells`.
- Реализовать Unicode NFKC/case-fold normalized index, `search`, next/previous, active match и highlight overlays.
- Реализовать public headless methods `renderPage`, `renderThumbnail`, `renderSheetViewport`, `getPageText`, `getDocumentInfo`.
- Добавить typed `ViewerEventMap` и unsubscribe-returning `on()`; частые view events throttled to one per animation frame.
- Экспортировать original source через `downloadOriginal` без подмены converted legacy bytes.

### Tests

- Type tests фиксируют public signatures, event payloads, zero-based indices и SSR-safe imports.
- Unit tests reducer/state transitions, zoom modes, cache eviction, visible range и coordinate transforms.
- Interaction tests mouse drag, wheel, pinch emulation, keyboard navigation, resize и DPR changes.
- Selection tests across multiple text runs/pages, mixed scripts, rotated pages и spreadsheet merged cells.
- Search tests Unicode normalization, Cyrillic case, CJK substring, Arabic diacritics-preserving behavior и Indic grapheme boundaries.
- Virtualization tests подтверждают bounded DOM/canvas count на документах в тысячи страниц/строк.
- Race tests rapid load/close, zoom while rendering, sheet change during search и cancellation stale results.
- Headless/browser parity tests для output dimensions, annotations flag и cancellation.

### Docs

- Создать `docs/api/reference.md` с public classes, types, methods, events и error catalog.
- Создать `docs/api/headless.md` с render/search/selection examples.
- Создать `docs/architecture/viewport.md` с coordinate spaces, virtualization и cache policy.
- Добавить migration note, объясняющий сходство и отличия от `@docmentis/udoc-viewer`.

## Критерии готовности

- Все format backends управляются одним public API без format casts.
- Pan/zoom/navigation не зависят от окончательного basic UI.
- Text/cell selection корректно копируется и управляется программно.
- Search работает по logical Unicode text и отображает активный match.
- Virtualization удерживает bounded resources на large documents.
- Public TypeScript contract покрыт type tests и документацией.
