# Задача 13c. DOC tables, media и serialization

**Статус:** 🟠 В работе с 2026-07-17

Core source-backed serialization готова: table grid строится по объединению
row edges, поддержаны spans/merges, widths, default cell padding, borders,
headers/footers, PAGE/NUMPAGES fields, footnotes/endnotes, point/ranged comments,
Word list tables и строгая media валидация. Публичный Word 97/2000/2003 corpus
конвертируется и повторно
открывается; private `.tmp` используется только как локальный oracle и никогда
не входит в fixture/package.

До завершения этапа остаются advanced/nested table properties, полноценные
floating/nested images, PICT/compressed metafile decode, экзотические list/style
варианты, custom note symbols и точная пагинация сложных документов.

## Цель

Восстановить таблицы и связанные document stories и сериализовать доказанную
структуру в DOCX для существующего renderer.

## Источники требований

- [`./13-legacy-doc-fidelity.md`](./13-legacy-doc-fidelity.md)
- [`./13b-doc-formatting-sections.md`](./13b-doc-formatting-sections.md)

## Что входит

### Feature

- Table paragraph properties (`fInTable`, `fTtp`, `itap`) и TAPX/TDefTable.
- Rows/cells, widths, padding, borders, shading, horizontal/vertical merges и
  nested tables с корректным paragraph order внутри cells.
- Header/footer stories и section linkage.
- Inline/anchored images, где payload и anchor подтверждаются source records;
  unsupported OLE objects остаются typed warning/placeholders без исполнения.
- Mapping в `office_oxide::ir::DocumentIR` и существующий permissive DOCX writer.
- Serializer audit: каждый emitted heading/table/section/property имеет source
  evidence; никаких duplicated titles.

### Tests

- Synthetic/public tables: uneven grids, merged and nested cells, borders,
  multi-paragraph cells, repeated headers и page-spanning rows.
- Media fixtures для supported raster payloads и malformed/OLE cases.
- Round-trip structural oracle по XML: grid, cells, order, widths, sections,
  headers/footers и relationships.
- Visual differential с независимо лицензированными reference renders.

### Docs

- DOC feature matrix для tables/media/headers/footers.
- Явный список unsupported embedded object types.

## Что не делаем

- Не запускаем macros, OLE packages или external links.
- Не rasterize весь документ как замену selectable layout.

## Критерии готовности

- Basic/merged/nested table grids сохраняются без flattening.
- DOCX output открывается существующим renderer и не содержит invented blocks.
- Malformed table/media records fail bounded и типизированно.

## Текущий результат

- Удалена прежняя paragraph-boundary ошибка по PAPX runs: логические абзацы
  теперь разделяются только source paragraph/cell marks.
- Реализованы heterogeneous row grids, `gridSpan`, source widths, borders,
  indent и cell margins; representative two-column + seven-column table больше
  не превращается в обычный текст.
- `sprmTFAutofit` теперь отличает fixed source grids от auto-fit tables;
  bridge добавляет `w:tblLayout type=fixed`. Знак `sprmTDyaRowHeight`
  сохраняется как `atLeast`/`exact`, а ноль остаётся content-derived. Для
  положительной minimum-height bridge использует `exact` только когда
  source-backed оценка ширины ячеек, полей, шрифта и текста доказывает
  однострочную строку; это не даёт OOXML renderer накапливать rounding slack,
  но сохраняет `atLeast` для реально переносящихся ячеек.
- Поля и notes читаются из story-specific PLCF и проецируются без instruction
  text; public footnote/endnote fixture проходит.
- `PlcfandRef`/`PlcfandTxt` и `GrpXstAtnOwners` валидируются bounded: сохраняются
  initials, полные имена авторов и тела комментариев. Point anchors проходят
  через IR-marker и заменяются на `w:commentReference`; bridge добавляет
  `word/comments.xml`, relationship и content-type override в памяти. Публичные
  Apache-2.0 fixtures проверяют один и три комментария и полный DOC→DOCX path.
- Реальный Apache POI comments fixture выявил допустимый нулевой байт
  выравнивания extended PAPX. Он принимается только для PAPX и только если вся
  предшествующая последовательность SPRM декодируется строго.
- `PlfLst`/`PlfLfo`, `LSTF`/`LVL`/`LFO`/`LFOLVL` теперь разбираются bounded.
  Абзац получает source `numId`/`ilvl`; bridge выпускает `numbering.xml`,
  `numPr`, start/format overrides, marker font/indent/tab, legal numbering и
  restart boundary. Public fixture доказывает 11 definitions, 11 instances и
  82 пронумерованных абзаца без private inputs.
- Ranged comments разрешаются через `SttbfAtnBkmk` и связанные
  `PlcfAtnBkf`/`PlcfAtnBkl`; bridge выпускает `commentRangeStart/End` вокруг
  source CP range. Отдельный Apache-2.0 fixture доказывает anchor `comment` на
  бинарном уровне и полный DOC→DOCX path. `sprmCFData` теперь типизированно
  отличает `NilPICFAndBinData` форм/гиперссылок от настоящего
  `PICFAndOfficeArtData`: 68-byte header и полный `lcb` валидируются, но binary
  payload не исполняется и не выдается за картинку.
- Unsupported PICT/compressed BLIP не обрушивает весь документ, но отдельный
  публичный warning channel для skipped media ещё требуется.
