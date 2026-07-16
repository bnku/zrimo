# Задача 04. PDF, images, CSV и SVG

**Статус:** ✅ Выполнена 2026-07-16

## Цель

Завершить format matrix безопасными adapters для PDF, raster/vector images и tabular text, сохранив общий API страниц, sheet viewports, search и selection maps.

## Источники требований

- [`../00-roadmap.md`](../00-roadmap.md)
- [`./02-runtime-security.md`](./02-runtime-security.md)
- [`./03-office-pipeline.md`](./03-office-pipeline.md)

## Зависимости

- Задачи 01–02 завершены.
- Общие render/text/cell contracts стабильны; Office pipeline можно использовать как reference adapter.

## Что входит в задачу

- PDF parsing, page rendering, metadata, text maps, thumbnails и existing visible annotations/forms в пределах renderer output.
- PNG, JPEG, WebP, GIF, BMP и TIFF.
- SVG с sanitization и external-resource policy.
- CSV/TSV как single-sheet spreadsheet backend.
- Unified warnings и headless render methods.

## Что НЕ делаем в этой задаче

- Не реализуем PDF editing, form filling, annotations editing или signatures.
- Не открываем encrypted/password-protected PDF.
- Не добавляем OCR для scanned PDF/images.
- Не поддерживаем HEIC/AVIF/RAW.
- Не вычисляем типы CSV агрессивно: исходные cell strings должны оставаться доступными.

## Основные задачи по слоям

### Feature

- Собрать минимальный `pdf_oxide` WASM backend с rendering/text-coordinate APIs и без native/CLI/multi-language bindings.
- Отдавать PDF page bitmap/ImageBitmap и text runs с Unicode text, glyph/range mapping и page coordinates.
- Реализовать page/thumbnail cache с pixel budget и cancellation.
- Использовать browser decoders для нативно поддерживаемых images; Rust `image` fallback включать только для TIFF/несовместимых вариантов.
- Представлять multi-page TIFF как paginated document; для animated GIF/WebP сохранять browser playback, когда используется native path.
- Санитизировать SVG: удалить scripts, event handlers, foreign active content и не разрешать external fetch без resolver.
- Парсить CSV/TSV в worker с delimiter/encoding detection, выдавать single sheet, cell map и TSV copy semantics.
- Унифицировать image/SVG как single-page document и поддержать pan/zoom/fit без text selection.

### Tests

- PDF golden corpus: fonts, CJK, RTL, transparency, images, rotations, crop boxes, links, forms и malformed objects.
- PDF text-map tests: selection order, copy text, mixed RTL/LTR, ligatures и page rotations.
- Image fixtures для каждой extension, EXIF orientation, large dimensions, animation и multi-page TIFF.
- SVG security fixtures для script, event attributes, data URLs, external image/font и malformed XML.
- CSV/TSV fixtures для delimiters, quoted newlines, BOM, UTF-8 Cyrillic/CJK/Arabic/Indic, empty cells и large sheets.
- Encrypted PDF возвращает `encrypted-document`; decompression/pixel bombs возвращают `resource-limit`.
- Headless render methods дают одинаковые output types и cancellation semantics во всех adapters.

### Docs

- Создать `docs/formats/pdf-images-data.md` с format/capability matrix и ограничениями.
- Документировать SVG security policy, image animation/TIFF behavior и CSV parsing rules.
- Добавить headless render examples для PDF page, image и CSV sheet viewport.

## Критерии готовности

- Вся согласованная non-Office matrix определяется и открывается через общий runtime.
- PDF предоставляет bitmap и корректную text map для следующего interaction этапа.
- SVG active content и внешние ресурсы заблокированы по умолчанию.
- CSV/TSV использует spreadsheet selection contract, а images — paginated viewport contract.
- Corrupted/adversarial fixtures не вызывают panic, hang или uncontrolled allocation.
- Golden, language, security и cleanup tests проходят.

## Результат

- В default registry добавлены PDF, PNG/JPEG/WebP/GIF/BMP, multi-page TIFF, sanitized SVG и CSV/TSV adapters; вся согласованная format matrix теперь маршрутизируется автоматически.
- PDF parsing/render/text выполняется persistent `pdf_oxide` WASM worker; PNG pages передаются transferable buffers, bounded LRU ограничен pixel budget, Unicode glyph boxes нормализованы в `TextRun`.
- Нативные raster formats используют browser decoder с EXIF orientation; отдельный project `image-wasm` на pinned permissive `image`/`tiff` обрабатывает TIFF IFD pages и aggregate pixel limits.
- SVG sanitizer блокирует active/foreign content, event handlers, CSS и все non-fragment resource references без сетевого resolver.
- CSV/TSV dependency-free parser работает в module worker, поддерживает quoted newlines/BOM/UTF-8/UTF-16/Windows-1252 fallback, сохраняет raw strings и выдаёт single-sheet cell map.
- 41 TypeScript unit/security/contract test и Rust multi-page/pixel-limit tests проходят; Chromium открывает PDF, PNG, TIFF, SVG, CSV и TSV через собранные package workers/assets.
- Capability и security matrix документированы в [`../../formats/pdf-images-data.md`](../../formats/pdf-images-data.md).
