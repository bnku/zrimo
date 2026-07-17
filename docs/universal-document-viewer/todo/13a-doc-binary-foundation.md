# Задача 13a. DOC binary foundation

**Статус:** ⬜ Запланирована

## Цель

Создать bounded Rust parser foundation для Word 97–2003 DOC, который выдаёт
проверяемые character positions и stories без потери или угадывания структуры.

## Источники требований

- [`./13-legacy-doc-fidelity.md`](./13-legacy-doc-fidelity.md)
- [`../legacy-doc-spike-decision.md`](../legacy-doc-spike-decision.md)

## Что входит

### Feature

- Новый crate `legacy-doc`, используемый `legacy-office-wasm` только для DOC.
- Public `office_oxide::cfb` как container reader; отдельные bounded readers для
  полной FIB field table, `0Table`/`1Table`, CLX/PCDT и CP↔FC mapping.
- Main text и story ranges; compressed ANSI через declared code page и Unicode
  pieces без lossy byte guessing.
- Typed errors для unsupported Word revisions, encryption и повреждённых offsets.
- Parser limits на streams, piece count, story length и allocation arithmetic.

### Tests

- Byte-level unit fixtures для FIB variants, CLX/PCD и mixed ANSI/Unicode pieces.
- Property/bounds tests на truncated PLCF, overflow, overlapping и descending CP.
- Public/provenance corpus минимум для Word 97, 2000, 2002 и 2003.
- Fuzz target FIB/CLX без panic, unbounded allocation или native dependency.

### Docs

- Описать supported Word revisions, code-page policy и typed error matrix.
- Зафиксировать provenance всех committed fixtures.

## Что не делаем

- Formatting, pagination и tables реализуются в следующих этапах.
- Word 6/95 не включается молча: только после отдельного format gate.

## Критерии готовности

- CP ranges однозначно восстанавливают source text и story boundaries.
- Некорректные offsets завершаются typed error до чтения вне stream.
- Crate собирается native и `wasm32-unknown-unknown` с permissive dependencies.
