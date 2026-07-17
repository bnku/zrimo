# Corrective roadmap: fidelity и viewport release blockers

**Статус:** 🟠 Requalification завершена; project-owned DOC parser в работе

## Назначение документа

Этот roadmap заменяет ошибочное предположение, что smoke-файла достаточно для
qualification форматного backend. Он фиксирует причины четырёх найденных
дефектов, целевую архитектуру исправлений и порядок задач `Feature → Tests →
Docs`. Исходный roadmap остаётся историей собранного alpha, но его release gates
считаются непройденными до задачи 14.

## Release blockers и доказанные причины

| Область        | Причина                                                                                                                                                                                                         | Решение                                                                                                                                                                          |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| DOCX selection | Общий text layer сохраняет только bounding box и теряет `font`, `fontSize`, `letterSpacingPx`, transform и vertical-text metadata. DOM-глифы поэтому не совпадают с canvas.                                     | Сохранить полный run contract и использовать экспортируемый `buildDocxTextLayer` из pinned `@silurus/ooxml`; унифицировать logical-offset mapping поверх format-specific layers. |
| PDF fonts      | `pdf_oxide` в браузерной WASM-сборке не может разрешить часть embedded Type 1/CFF/CID и standard fonts. Его последний fallback намеренно рисует прямоугольник на символ.                                        | Перенести PDF display/text backend на Apache-2.0 `pdfjs-dist`; Rust/WASM PDF renderer вывести из production path.                                                                |
| Legacy DOC     | `office_oxide@0.1.6` извлекает из DOC только plain text и images, а DOC→IR эвристически превращает короткие/all-caps строки в headings. Таблицы, секции и formatting в этом pipeline отсутствуют принципиально. | Немедленно прекратить выдавать lossy projection за faithful render; затем расширить permissive Rust parser/IR для структуры Word Binary и нормализовать её в DOCX.               |
| XLSX viewport  | Один paginated viewport с базой `816×1056` используется и для sheets. Scroll не меняет `sheetRange`, а spacer ограничен размером условной страницы.                                                             | Выделить самостоятельный `SpreadsheetViewport`: полный used-range extent, variable row/column geometry, scroll→visible range/tile mapping и viewport-sized canvas.               |

## Зафиксированные архитектурные решения

### Text layer

- Canvas остаётся визуальным source of truth; selectable DOM обязан повторять
  те же font metrics и transforms.
- `TextRun` получает достаточную геометрию для glyph placement: CSS font
  shorthand/font size, letter spacing, transform/writing mode и явные logical
  offsets либо cluster map.
- DOCX использует upstream `buildDocxTextLayer`, а не ещё одну копию алгоритма.
- PDF использует text content/text-layer primitives того же PDF.js document,
  которым отрисован canvas.
- Search highlight отделяется от прозрачного selectable text, чтобы background
  span не менял hit testing.

### PDF

- Рекомендуемый production backend — `pdfjs-dist`: worker, page render,
  `getTextContent`, links и page geometry загружаются лениво как отдельный
  format chunk.
- `cMapUrl`, `standardFontDataUrl`, worker URL и optional ICC assets разрешаются
  через существующий `assetBaseUrl`; сетевой доступ к сторонним CDN не нужен.
- `pdf_oxide` можно оставить только на время migration для differential tests.
  Два render backend в релизе не сохраняются без отдельной измеримой причины.
- Alternative — доработка Type 1/CFF/CID и font fallback в `pdf_oxide` —
  отклонена как основной путь: она существенно дороже и дублирует зрелый
  browser renderer. Вернуться к ней можно только если PDF.js не проходит size,
  security или supported-browser gates.

Официальный PDF.js предоставляет page render и `getTextContent`, а его основной
репозиторий и npm distribution имеют Apache-2.0 license:
[API](https://mozilla.github.io/pdf.js/api/),
[repository](https://github.com/mozilla/pdf.js),
[npm setup](https://github.com/mozilla/pdf.js/wiki/Setup-pdf.js-in-a-website).

### Legacy DOC

- Текущий DOC→plain-text→heuristic IR→DOCX путь удаляется из режима faithful
  rendering. До готовности нового backend DOC либо открывается в явно помеченном
  `text-fallback`, либо возвращает typed `fidelity-unsupported` outcome; выбор
  фиксируется в задаче 13 до изменения public warning/error contract.
- Целевой путь остаётся permissive и browser-side: форк/расширение
  `office_oxide` использует существующие CFB/FIB/CLX части и добавляет STSH,
  PAPX/CHPX FKP, section properties, paragraph/run properties и Word table
  properties. Результат — структурный IR без эвристического угадывания, затем
  OOXML serialization и существующий DOCX renderer.
- Mature copyleft converters не становятся runtime dependency. Если spike не
  подтверждает practical fidelity в заданном budget, legacy DOC остаётся
  незавершённым release blocker, а не маскируется красивым plain-text output.

Upstream `office_oxide` permissive, но его текущий DOC model публично хранит
только extracted text/images; поэтому факт успешного parsing не равен layout
fidelity: [repository](https://github.com/yfedoseev/office_oxide),
[DOC source](https://github.com/yfedoseev/office_oxide/blob/main/src/doc/document.rs).

### Spreadsheet viewport

- Paginated `ViewerViewport` обслуживает pages/slides/images; sheets получают
  отдельный layout strategy внутри общего `DocumentViewer` API.
- `SpreadsheetWorksheet` сохраняет `colWidths`, `rowHeights`, defaults, hidden
  bands, merged cells, freeze panes и used-range boundaries из
  `@silurus/ooxml`.
- Prefix sums с binary search (или эквивалентный индекс) переводят scroll offset
  в row/column range с overscan; canvas равен видимой области, а spacer — полной
  логической области листа.
- `renderViewport` получает актуальные range, `scrollOffsetX/Y`, zoom и freeze
  geometry. `fitWidth`/`fitPage` считаются по used range, а zoom сохраняет точку
  под курсором/центром viewport.
- Встраивать целиком `XlsxViewer` не планируется: он создаёт собственные tabs и
  controls. Переиспользуются `XlsxWorkbook`, worksheet geometry и renderer.

## Политика приватных regression-файлов

- Входные файлы, переданные для диагностики, остаются только в ignored
  `.tmp/**`. Их bytes, имена, hashes, извлечённый текст, screenshots и
  производные документы не коммитятся и не попадают в npm tarball, test corpus,
  reports, docs или examples.
- Локальный opt-in regression runner получает внешний каталог через environment
  variable, выдаёт только case-id/pass/fail и не сохраняет render output в repo.
- Committed regression fixtures создаются независимо: synthetic generators либо
  public fixtures с проверенной provenance/license. Нельзя минимизировать или
  модифицировать приватный файл и затем коммитить результат.
- Pack gate проверяет allowlist и дополнительно отклоняет private/corpus/temp
  paths и известные release-forbidden extensions независимо от `.gitignore`.

## Порядок задач

### Задача 09. Release quarantine и regression harness

**Статус:** ✅ Завершена 2026-07-17

Закрыть ложный promotion status, усилить pack scanner и создать private-local и
committed-synthetic test lanes.

Результат: добавлены machine-readable quarantine и fail-closed promotion gate,
проверяемая qualification matrix с четырьмя явными `unsupported` oracles,
local-only private lane, recursive tarball scanner и sentinel test. Pack
кандидат теперь создаётся только в ignored `.cache`, не заменяя release artifact.

Подробности: [`./todo/09-release-quarantine-regression-harness.md`](./todo/09-release-quarantine-regression-harness.md)

### Задача 10. DOCX text layer и selection mapping

**Статус:** ✅ Завершена 2026-07-17

Сохранить точные run metrics, подключить upstream overlay и доказать корректное
выделение/copy на разных zoom и scripts.

Результат: DOCX runs сохраняют полный renderer contract и logical UTF-16
offsets; viewport лениво использует upstream selection/highlight builders,
масштабирует natural coordinate layer целиком и исправляет native Range по
grapheme boundaries. Geometry/copy/drag/double-click tests проходят в Chromium,
Firefox и WebKit, включая отдельный Chromium DPR 2 project.

Подробности: [`./todo/10-docx-text-layer-selection.md`](./todo/10-docx-text-layer-selection.md)

### Задача 11. XLSX spreadsheet viewport

**Статус:** ✅ Завершена 2026-07-17

Отделить sheet scrolling/virtualization от page layout и устранить clipping при
zoom/scroll.

Результат: `AdaptiveViewport` маршрутизирует sheets в отдельный
`SpreadsheetViewport` с full-used-range spacer, sparse variable-axis indexes,
одним bounded canvas, partial-cell offsets и frozen panes. Detached-frame commit
отбрасывает stale renders; synthetic `10 000×1 000` E2E проходит zoom matrix
`0.25–4`, deep scroll, merge/selection и bounded DOM без A4 constants.

Подробности: [`./todo/11-xlsx-spreadsheet-viewport.md`](./todo/11-xlsx-spreadsheet-viewport.md)

### Задача 12. PDF.js display backend и font fidelity

**Статус:** ✅ Завершена 2026-07-17

Заменить rectangle fallback на browser-proven PDF renderer с локальными font/CMap
assets и единым canvas/text source.

Результат: production adapter переведён на `pdfjs-dist@6.1.200` с explicit
module worker и локальными CMap/standard-font/WASM/ICC assets. Canvas и text
overlay используют один PDF.js viewport; cancellation, limits, links и cleanup
сохраняют typed contract. Старый Rust renderer/PNG worker/cache удалён из
workspace и npm artifact. Public Type1/CFF/CID/TrueType/Arabic/JIS/Base-14 corpus
и browser matrix не обнаруживают rectangle fallback или внешний network.
Compatibility follow-up закрепил matching legacy runtime/worker с worker cache
versioning и актуальный PDF.js 6 point-based API для link annotations.

Подробности: [`./todo/12-pdf-font-rendering.md`](./todo/12-pdf-font-rendering.md)

### Задача 13. Legacy DOC structured conversion

**Статус:** 🟠 В работе: stock converter отклонён, новый parser утверждён

Придуманные headings удалены из runtime path: TypeScript и Rust fail closed с
`fidelity-unsupported`. Spike подтвердил, что structured Word Binary pipeline
требует нового parser scope. Реализация разбита на bounded foundation,
formatting/sections, tables/media/serialization и browser qualification; до
завершения этих этапов DOC остаётся blocker.

Подробности: [`./todo/13-legacy-doc-fidelity.md`](./todo/13-legacy-doc-fidelity.md)

### Задача 14. Cross-format requalification и новый alpha

**Статус:** 🟠 Выполнена для qualified matrix; promotion заблокирован задачей 13

Fidelity, browser, performance, license, size и clean-pack gates повторно
пройдены для qualified matrix. Локальный `0.1.0-alpha.1` не является release
candidate, пока DOC structural gate остаётся `unsupported`.

Подробности: [`./todo/14-fidelity-requalification.md`](./todo/14-fidelity-requalification.md)

## Последовательность и оценка

1. Задача 09 — 1–2 рабочих дня и обязательна перед любым новым artifact.
2. Задачи 10 и 11 — по 2–4 и 4–7 рабочих дней; независимы после задачи 09.
3. Задача 12 — 5–8 рабочих дней, включая worker/assets/selection migration.
4. Задача 13a — 4–6 рабочих дней на bounded FIB/CLX foundation.
5. Задача 13b — 7–10 рабочих дней на formatting, styles и sections.
6. Задача 13c — 10–15 рабочих дней на tables, media и serialization.
7. Задача 13d — 5–8 рабочих дней на browser integration и qualification.
8. Задача 14 — 4–7 рабочих дней после завершения всех выбранных release
   форматов.

Оценки — engineering ranges, а не обещание календарной даты. Legacy DOC задаёт
critical path.

## Promotion gates

- Selection rectangles для каждого тестового run пересекают visual glyph bounds
  минимум на 90%, copy сохраняет logical order и grapheme boundaries.
- PDF corpus покрывает embedded/subset Type 1, CFF, TrueType, CID Type 0,
  non-embedded standard fonts, CJK, RTL, rotations и transparency; прямоугольный
  last-resort output считается hard failure, а не warning.
- DOC tables сохраняют row/cell boundaries, widths, borders, paragraphs внутри
  cells, sections и page geometry; converter не создаёт heading/title без
  исходного style evidence.
- XLSX на zoom `0.25–4.0` позволяет достигнуть последней used row/column без
  clipping; resize/zoom/scroll не возвращают stale tile и не привязаны к A4.
- Browser matrix, security limits, lifecycle cleanup, license allowlist и size
  budget проходят на tarball, собранном из clean checkout.
- `npm pack --dry-run` и распаковка tarball подтверждают отсутствие всех private
  inputs и regression outputs.
