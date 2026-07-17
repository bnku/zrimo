# Задача 11. XLSX spreadsheet viewport

**Статус:** ✅ Завершена 2026-07-17

## Фактический результат

`AdaptiveViewport` теперь выбирает самостоятельный `SpreadsheetViewport` для
`unit: "sheet"`; page constants, slots и gaps в этот путь не входят. Лист имеет
full-used-range spacer, один sticky viewport-sized canvas и DOM overlay
выделения. Sparse prefix-delta indexes с binary search учитывают default,
custom и hidden row/column sizes; текущий scroll переводится в bounded range с
overscan и частичными `scrollOffsetX/Y`. Frozen panes передаются закреплённому
OOXML renderer.

Рендер выполняется в detached canvas и коммитится только по актуальному
generation token, поэтому late completion не может затереть новый
scroll/zoom/resize frame даже при игнорировании abort backend-ом. Zoom сохраняет
pointer/center anchor, fit считается по used range, состояние scroll хранится
по листам, cell hit testing использует тот же geometry index. Полная cell map
строится из sparse worksheet model и больше не ограничена первыми `200×50`.

Проверки: unit tests покрывают million-band sparse geometry, hidden/custom
sizes, полную text map и propagation partial offsets. Chromium E2E использует
независимый synthetic sheet `10 000×1 000` с freeze/merge/hidden/custom bands и
проверяет zoom `0.25/0.5/1/2/4`, достижимость последней cell, bounded canvas/DOM,
stale-frame dropping и selection после deep scroll. Приватные regression-файлы
не используются.

## Цель

Удалить A4/page assumptions из просмотра sheets и обеспечить полный scrollable
used range при любом zoom без обрезания и stale renders.

## Источники требований

- [`../01-fidelity-corrective-roadmap.md`](../01-fidelity-corrective-roadmap.md)
- [`./05-viewport-interactions-api.md`](./05-viewport-interactions-api.md)

## Зависимости

- Задача 09 завершена.

## Что входит в задачу

- Отдельный `SpreadsheetViewport` за тем же `DocumentViewer` API.
- Variable row/column geometry, used range и full scroll extent.
- Visible range/tile rendering, frozen panes и cell selection.
- Spreadsheet-specific fit/zoom/navigation semantics.

## Что НЕ делаем в этой задаче

- Не создаём DOM element на каждую cell.
- Не превращаем sheet в набор печатных A4 pages.
- Не реализуем formula calculation или редактирование cells.

## Основные задачи по слоям

### Feature

- Выбирать layout strategy по `DocumentInfo.unit`: `sheet` не проходит через
  `BASE_WIDTH/BASE_HEIGHT`, page slots или page-gap calculations.
- Протянуть из `Worksheet` widths/heights/defaults, hidden bands, merges, freeze
  panes и used bounds; не обрезать их локальным `SpreadsheetWorksheet` type.
- Построить prefix-sum geometry index и binary-search offset→row/column.
  Учитывать zoom, headers, custom/hidden sizes и overscan.
- Spacer задаёт полный logical extent used range; один viewport-sized canvas
  рендерит актуальный range с `scrollOffsetX/Y`, generation token и cancellation.
- Синхронизировать cell text/selection overlay с тем же range. Frozen panes и
  headers остаются фиксированы; scrollable body не дублируется.
- Реализовать sheet-specific `fitWidth`, `fitPage`, pointer-anchored zoom,
  `panBy`, resize и смену sheet с восстановлением/сбросом scroll policy.

### Tests

- Synthetic sheets с данными за пределами 100 rows/30 columns, custom sizes,
  hidden bands, merges, images, freeze rows/columns и sparse far-away cells.
- Zoom matrix `0.25`, `0.5`, `1`, `2`, `4`: достижима последняя used cell по
  обеим осям, canvas/spacer не равны A4 constants.
- Scroll/zoom/resize race tests подтверждают stale-frame dropping и отсутствие
  blank/clipped bottom/right regions.
- Cell drag/Shift+arrow/copy tests после horizontal/vertical scroll и zoom.
- Large sparse sheet удерживает bounded canvas/DOM/memory и не сканирует каждый
  row/column на каждый frame.

### Docs

- Разделить page и sheet coordinate spaces в viewport architecture.
- Описать sheet fit semantics, used-range policy, frozen panes и scroll events.
- Обновить headless/UI examples для большого листа.

## Критерии готовности

- Ни один spreadsheet UI path не использует page/A4 constants.
- Пользователь может дойти до последней used row/column на всей zoom matrix.
- Canvas всегда соответствует видимой области/range, а не фиксированным первым
  100×30 cells.
- Selection, fit, pan и sheet switching сохраняют public API compatibility.
