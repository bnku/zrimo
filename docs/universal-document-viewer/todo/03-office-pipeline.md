# Задача 03. Modern и legacy Office pipeline

**Статус:** ✅ Выполнена 2026-07-16

## Цель

Подключить DOCX/XLSX/PPTX rendering и построить единый legacy DOC/XLS/PPT путь через permissive normalization в OOXML, чтобы modern и legacy документы использовали одинаковые viewer, text map и interaction semantics.

## Источники требований

- [`../00-roadmap.md`](../00-roadmap.md)
- [`./01-foundation-dependency-qualification.md`](./01-foundation-dependency-qualification.md)
- [`./02-runtime-security.md`](./02-runtime-security.md)

## Зависимости

- Задачи 01–02 завершены.
- Pinned `@silurus/ooxml` и `office_oxide` прошли qualification.

## Что входит в задачу

- DOCX/DOCM, XLSX/XLSM, PPTX/PPTM/PPSX adapters.
- DOC→DOCX, XLS→XLSX и PPT→PPTX conversion в worker через bytes-in/bytes-out Rust API.
- Единые document metadata, page/slide/sheet models, text maps и hyperlinks.
- Spreadsheet cached values, sheets, merged cells, frozen panes и cell ranges в пределах возможностей backend.
- Warnings для unsupported/degraded Office features.

## Что НЕ делаем в этой задаче

- Не выполняем VBA/macros и formulas.
- Не редактируем и не сохраняем пользовательские изменения.
- Не открываем password-protected Office documents.
- Не добавляем ODT/ODS/ODP или старые pre-OLE Office formats.
- Не маскируем fidelity degradation: она должна попадать в `DocumentInfo.warnings` и event stream.

## Основные задачи по слоям

### Feature

- Обернуть headless `@silurus/ooxml` engines в общий `DocumentBackend` без утечки format-specific classes в public API.
- Нормализовать pages/slides в paginated units, spreadsheets — в sheet/viewport units.
- Реализовать Rust WASM binding `convertToOoxmlBytes(input, legacyFormat)` поверх pinned `office_oxide` IR/create APIs.
- Передавать converted OOXML bytes напрямую следующему adapter без Blob URL и повторного network fetch.
- Игнорировать VBA parts и выдавать informational warning о наличии macro content.
- Для XLS/XLSX показывать cached formula result; при отсутствии cached value показывать формулу как text и warning, но не вычислять её.
- Блокировать external relationships; разрешать host-controlled resource resolver только как отдельную opt-in возможность.
- Нормализовать internal/external hyperlinks и отдавать их через sanitized hit regions/callback.
- Сохранять original bytes для download, а converted bytes считать временным derived artifact и освобождать при `close`.

### Tests

- Golden tests для headers/footers, sections, tables, lists, images, text wrap, masters, shapes, charts, merged cells и sheet styles.
- Legacy pair tests: исходный DOC/XLS/PPT и converted OOXML дают ожидаемые content/structure и проходят legacy SSIM threshold.
- Macro-enabled fixtures открываются, отображают document content и не выполняют VBA/network side effects.
- Formula fixtures показывают cached results; absence of cache диагностируется без вычисления.
- Hyperlink tests для internal targets и разрешённых/запрещённых external schemes.
- Corrupted ZIP/CFB, encrypted Office и unsupported embedded object fixtures возвращают стабильные errors/warnings без panic.
- Memory tests подтверждают освобождение original/conversion/intermediate models после close/destroy.

### Docs

- Создать `docs/formats/office.md` с feature matrix отдельно для modern и legacy formats.
- Документировать macro/formula/external-resource policy и known fidelity limitations.
- Обновить dependency documentation ссылкой на upstream patch/fork bytes-out binding.

## Критерии готовности

- Все девять согласованных Office extensions маршрутизируются автоматически.
- Modern Office и соответствующий legacy format имеют одинаковые navigation/search/selection semantics.
- Legacy conversion работает полностью в browser worker и не использует server/native executable.
- Macros никогда не выполняются, cached formula policy подтверждена тестами.
- Golden/structure/security tests проходят установленные thresholds.
- Temporary converted bytes и parser handles освобождаются при close/destroy.

## Результат

- Реализован автоматически зарегистрированный `OfficeDocumentAdapter` для DOCX/DOCM, XLSX/XLSM, PPTX/PPTM/PPSX и DOC/XLS/PPT.
- Legacy bytes конвертируются отдельным module worker через project-owned `office_oxide` WASM binding и transferable buffers, после чего передаются тому же OOXML backend без сети и временных файлов.
- Унифицированы page/slide/sheet metadata, text maps, RTL direction, internal targets и allowlist-sanitized external hyperlinks.
- Для spreadsheets экспортируются merged ranges, frozen panes и used bounds; формулы удаляются из render model, cached values сохраняются, отсутствие cache показывает текст формулы и warning.
- Macro content никогда не исполняется; encrypted Office получает стабильный `encrypted-document`; conversion/parser handles освобождаются lifecycle-методами.
- 30 TypeScript unit/contract tests проходят; Chromium открывает DOCX/XLSX/PPTX и полный DOC/XLS/PPT worker-conversion pipeline. Native qualification повторно проверяет bytes-out и reparsing всех трёх legacy families.
- Политики и feature matrix документированы в [`../../formats/office.md`](../../formats/office.md).
