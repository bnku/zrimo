# Задача 13b. DOC formatting и sections

**Статус:** ⬜ Запланирована после 13a

## Цель

Восстановить подтверждённые source properties для runs, paragraphs и sections,
чтобы DOCX renderer получал структуру, а не plain-text projection.

## Источники требований

- [`./13-legacy-doc-fidelity.md`](./13-legacy-doc-fidelity.md)
- [`./13a-doc-binary-foundation.md`](./13a-doc-binary-foundation.md)

## Что входит

### Feature

- STSH styles и inheritance с cycle/bounds protection.
- BTE PLCF lookup, PAPX/CHPX FKP pages и versioned `sprm` decoder.
- Run properties: font, size, bold/italic/underline, color, highlight, language,
  bidi и character spacing.
- Paragraph properties: alignment, indents, spacing, tabs, keep/page-break,
  outline evidence и list references.
- PLCFSED/SEPX: section breaks, page size/orientation/margins и columns.
- Mapping только известных properties в generic `DocumentIR`; неизвестные SPRM
  дают structured warning и сохраняют text order.

### Tests

- Unit tests по каждому поддержанному SPRM и style inheritance.
- Structural fixtures для mixed styles, Cyrillic/Latin/CJK/Arabic/Indic, RTL,
  page geometry, columns, lists и section breaks.
- Hard failures на invented heading/title и несуществующий page break.
- Fuzz/property tests PLCF/FKP/sprm length and offset arithmetic.

### Docs

- Capability table поддержанных SPRM/property mappings.
- Warning contract для корректно проигнорированных source properties.

## Что не делаем

- Table reconstruction и embedded objects относятся к 13c.
- Не выводим heading из all-caps/длины строки без source outline/style evidence.

## Критерии готовности

- Character/paragraph runs совпадают с source CP ranges.
- Sections и page geometry проходят structural oracle.
- Unsupported properties не создают fabricated OOXML.
