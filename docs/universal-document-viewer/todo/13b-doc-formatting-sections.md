# Задача 13b. DOC formatting и sections

**Статус:** ✅ Завершена 2026-07-17

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

## Результат

- Реализован bounded parser `PlcBteChpx`/`PlcBtePapx` и 512-byte
  `ChpxFkp`/`PapxFkp`, включая обычную и extended форму `PapxInFkp`.
- Direct formatting сохраняется как source-backed FC ranges, paragraph style
  index и точные raw `grpprl`; лимиты введены отдельно для FKP pages и runs.
- Generic Word 97+ SPRM framing декодирует `ispmd`/`fSpec`/`sgc`/`spra`, все
  fixed operand sizes, обычные variable operands и двухбайтовую длину
  `sprmTDefTable`, а также обычную и extended `sprmPChgTabs`. Неизвестные opcode
  сохраняются, а не превращаются в догаданные свойства.
- CLX `Prc` больше не отбрасываются: `Prm0` и `Prm1` применяются после FKP к
  соответствующей property family. FC ranges точно split-ятся по piece и
  paragraph boundaries в source CP ranges.
- Типизированы подтверждённые character/paragraph properties: font slots,
  size, emphasis, underline, colors, highlight, language, bidi, spacing,
  alignment, indents, line/paragraph spacing, tabs, keep/page-break, outline,
  lists и table-depth markers. Style-relative toggles не схлопываются до STSH.
- Реализованы `STSH`/`STD`/`UPX`, Unicode style names, default fonts, base/next/
  linked references и inheritance с kind/bounds/cycle protection. Итоговый
  formatting order: STSH defaults → paragraph style → character style →
  CHPX/PAPX → piece PRM.
- `SttbfFfn` сохраняет source font name, alternate name, charset, weight,
  PANOSE и Unicode/code-page signature для последующего font fallback.
- `PLCFSED`/`SED`/`SEPX` восстанавливают source section CP ranges, page size,
  orientation, margins, columns, header/footer distances и bidi geometry.
- 33 unit tests и публичный Word 97/2000/2002/2003 corpus проверяют FKP, PRM,
  STSH inheritance, font tables, semantic/style composition и sections.
  `cargo clippy -D warnings`, dedicated fuzz build, полный `npm run check`,
  qualification suite и release WASM build проходят.

Runtime всё ещё обязан возвращать `fidelity-unsupported`: таблицы, связанные
headers/footers, media и DOCX serialization относятся к 13c.
