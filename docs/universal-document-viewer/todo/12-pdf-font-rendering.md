# Задача 12. PDF.js display backend и font fidelity

**Статус:** ✅ Завершена 2026-07-17

## Фактический результат

Production PDF path переведён на pinned Apache-2.0
`pdfjs-dist@6.1.200`. Adapter лениво создаёт explicit module worker, передаёт
ему defensive copy входных bytes и локальные URLs packed CMaps, standard fonts,
OpenJPEG/JBIG2/QCMS WASM и ICC assets. System-font fallback, XFA и eval отключены;
Base-14 fonts детерминированно берутся из package assets. `assetBaseUrl` и все
directory overrides поддерживают self-hosting без CDN.

`PDFPageProxy.render()` пишет прямо в target canvas с zoom/DPR transform и
проверкой фактического pixel count; PNG encode/decode и LRU bitmap cache удалены.
`RenderTask.cancel()` связан с operation signal. Loading task, pages, fonts,
document и worker освобождаются на close/destroy. Password, malformed,
resource-limit, cancellation и render errors сохраняют typed contract.

Canvas и selectable overlay строятся из одного PDF.js viewport.
`getTextContent()` нормализуется в `TextRun` с font family/size, rotation,
direction, page extent и logical offsets; PDF-specific DOM layer применяет
renderer width через `scaleX`. Link annotations проходят scheme allowlist,
internal destinations получают page index, когда он разрешим. Viewer FontManager
не подменяет PDF.js generated/subset families.

Удалены `crates/pdf-wasm`, production `pdf_oxide`, `pdf-worker.ts`, PDF WASM,
старый worker, PNG bridge и release cache. Parser-only `pdf_oxide` остаётся лишь
в отдельном cargo-fuzz workspace и не входит в сборку/package.

Проверки: public Apache-2.0 corpus из pinned PDF.js revision покрывает Type 1,
CFF/CID, complex TrueType, Arabic CID TrueType, non-embedded JIS/CMap и Base-14
standard fonts. Chromium проверяет отсутствие повторяющихся solid rectangle
fallbacks, nonblank output, text, cancellation, local-only worker/CMap/font
requests и PDF selection overlay; тот же suite включён в Chromium DPR2,
Firefox и WebKit matrix. PDF visual golden обновлён на новый backend. Приватные
diagnostic files не используются и не сохраняются.

## Цель

Заменить неполный WASM font rasterization path на permissive browser PDF backend,
корректно отображающий embedded/subset/standard fonts и отдающий согласованный
text layer.

## Источники требований

- [`../01-fidelity-corrective-roadmap.md`](../01-fidelity-corrective-roadmap.md)
- [`./04-pdf-images-csv-svg.md`](./04-pdf-images-csv-svg.md)

## Зависимости

- Задача 09 завершена.
- Text-layer strategy задачи 10 согласован; реализация может идти независимо.

## Что входит в задачу

- Qualification и lazy integration `pdfjs-dist`.
- Worker, CMap, standard-font и optional ICC asset resolution.
- Canvas render, text content, links, page geometry и cancellation.
- Удаление rectangle fallback из production path.

## Что НЕ делаем в этой задаче

- Не встраиваем полный stock PDF.js viewer/UI.
- Не добавляем editing, annotations editing, signatures или password flow.
- Не загружаем assets с внешнего CDN по умолчанию.

## Основные задачи по слоям

### Feature

- Зафиксировать версию `pdfjs-dist`, Apache-2.0 notices, supported browsers,
  unpacked/Brotli size и security surface до включения runtime dependency.
- Реализовать `PdfBackend` поверх `getDocument`/worker: page count/size,
  `page.render`, `getTextContent`, annotations/links и deterministic cleanup.
- Поставлять worker, CMaps, standard fonts и нужные ICC assets через package
  allowlist и `assetBaseUrl`; worker/API versions проверять на совпадение.
- Рендерить непосредственно в target canvas с DPR/zoom, отменять `RenderTask` и
  освобождать page/document resources. Не делать промежуточный PNG encode/decode.
- Адаптировать PDF text items с transform/font styles/dir в format-specific text
  layer и logical text map.
- Удалить `pdf_oxide` render worker/WASM/assets/cache после parity, либо оставить
  только явно обоснованный не-rendering модуль с отдельным size/license gate.

### Tests

- Font corpus: embedded/subset Type 1, CFF, TrueType/OpenType, Type 0 CID,
  non-embedded base-14, custom encodings, ToUnicode/no-ToUnicode, CJK и Arabic.
- Visual goldens для rotations, crop/media boxes, transparency, gradients,
  images и mixed fonts; rectangle-shaped glyph fallback — hard failure.
- Text selection/search/copy для ligatures, combining marks, Bidi и rotated text.
- Worker URL/self-host/offline tests, cancellation, rapid close/load, memory
  cleanup и no-third-party-network assertion.
- Encrypted/malformed/resource-limit behavior сохраняет typed contract.
- Bundle/parse/startup benchmark сравнивается с alpha и проходит общий size gate.

### Docs

- Обновить PDF architecture, asset deployment и CSP/worker requirements.
- Обновить third-party notices, SBOM, size report и troubleshooting fonts/CMaps.
- Удалить утверждения, что production PDF rasterization выполняет `pdf_oxide`,
  после migration.

## Критерии готовности

- Font corpus не содержит квадратов из backend fallback и проходит visual/text
  gates во всех supported browsers.
- Canvas и text layer происходят из одного PDF.js page/viewport transform.
- Offline/self-host mode не делает внешних запросов.
- Старый Rust rendering path отсутствует в release artifact либо имеет отдельно
  доказанную необходимую функцию, не дублирующую display backend.
