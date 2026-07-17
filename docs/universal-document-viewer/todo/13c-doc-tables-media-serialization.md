# Задача 13c. DOC tables, media и serialization

**Статус:** ⬜ Запланирована после 13b

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
