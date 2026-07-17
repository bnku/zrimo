# Задача 13a. DOC binary foundation

**Статус:** ✅ Завершена 2026-07-17

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
- Main text и story ranges; compressed pieces через нормативную
  `FcCompressed` mapping table и Unicode pieces без lossy byte guessing.
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

## Результат

- Добавлен workspace crate `legacy-doc`. CFB-контейнер читает permissive
  `office_oxide::cfb`; FIB, `0Table`/`1Table`, CLX/Pcdt/PlcPcd и CP↔FC mapping
  разбираются собственными checked readers.
- FIB читается по переменным count-полям, а не по фиксированным offsets. В IR
  foundation сохраняются глобальные CP ranges main, footnote, header/footer,
  comment, endnote и textbox stories, а также исходные UTF-16 units.
- Все physical ranges проверяются против meaningful `cbMac` до decode.
  Ограничены input/stream bytes, FIB pairs, piece count и суммарное число CP;
  arithmetic выполняется checked-операциями.
- Добавлен отдельный `legacy_doc` fuzz target для CFB path и напрямую поданных
  Word/table streams. Короткий sanitizer smoke выполнил более 1,3 млн inputs без
  panic; target включён в общий Rust fuzz gate.
- Native tests, strict Clippy, `wasm32-unknown-unknown --release` и общий
  qualification gate проходят. Публичный runtime всё ещё fail closed: этот этап
  намеренно не сериализует частичный текст как faithful DOC.

## Поддерживаемые revisions и text policy

| `nFib` | Семейство | Gate |
| --- | --- | --- |
| `0x00C1` | Word 97 | public corpus |
| `0x00D9` | Word 2000 | public corpus |
| `0x0101` | Word 2002 | public corpus |
| `0x010C` | Word 2003 | public corpus |
| `0x0112` | поздний binary DOC | принимается и покрыт public corpus |

Word 6/95 не проходит этот gate и возвращает `UnsupportedVersion`. Для
uncompressed pieces одна CP соответствует одному UTF-16 code unit. Для
compressed pieces используется определённая MS-DOC 8-bit→Unicode mapping;
произвольная CP1252/locale эвристика запрещена. Font charset и formatting,
которые нужны для 13b, не выводятся из текста задним числом.

## Typed error matrix

| Класс входа | Ошибка |
| --- | --- |
| oversized input/stream | `InputTooLarge` / `StreamTooLarge` |
| отсутствующий CFB stream | `MissingStream` |
| повреждённый CFB | `CompoundFile` |
| invalid FIB/counts/table selection | `InvalidFib` |
| Word 6/95 или неизвестная revision | `UnsupportedVersion` |
| encrypted/obfuscated DOC | `PasswordProtected` |
| invalid CLX/Pcdt/PlcPcd | `InvalidPieceTable` |
| offset за границей stream | `OutOfBounds` |
| превышенный count/CP budget | `ResourceLimit` |

## Corpus provenance

Qualification использует pinned Apache POI fixtures под Apache-2.0 для Word
97, 2000, 2002, 2003, таблицы, Unicode headers/footers, footnotes и негативного
Word 6 gate. URL, commit и SHA-256 каждого файла находятся в
[`../../../tests/corpus/manifest.json`](../../../tests/corpus/manifest.json).
Бинарные fixtures загружаются только в ignored `.cache/corpus` и не входят в
репозиторий или npm artifact.
