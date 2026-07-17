# Roadmap: Zrimo v1

> **Release correction, 2026-07-17:** alpha qualification выявила четыре
> блокирующих класса fidelity-дефектов в DOCX selection, PDF fonts, legacy DOC
> layout и XLSX viewport. До завершения
> [`01-fidelity-corrective-roadmap.md`](./01-fidelity-corrective-roadmap.md)
> текущий alpha artifact не является кандидатом на promotion в beta/1.0.

## Назначение документа

Этот roadmap фиксирует согласованную последовательность разработки framework-agnostic npm-пакета для полностью клиентского просмотра документов. Пакет должен скрывать форматные Rust/WASM-модули за единым TypeScript API и предоставлять готовый базовый viewer с pan, zoom, навигацией, поиском и выделением.

Документ является главным индексом feature-пакета `universal-document-viewer`. Репозиторий на момент планирования пуст, поэтому roadmap одновременно задаёт начальную структуру workspace, публичные контракты и quality gates.

## Источники требований

- Согласованное описание продукта в planning-сессии от 2026-07-16.
- Референс API и UX: [`@docmentis/udoc-viewer`](https://www.npmjs.com/package/@docmentis/udoc-viewer).
- Modern Office renderer: [`@silurus/ooxml`](https://github.com/yukiyokotani/office-open-xml-viewer).
- Legacy Office parser/converter: [`office_oxide`](https://github.com/yfedoseev/office_oxide).
- PDF backend: [`pdf_oxide`](https://github.com/yfedoseev/pdf_oxide).

## Цель v1

Поставить ESM npm-пакет `@zrimo/viewer`, который:

- работает в браузере без серверной конвертации и не отправляет документы во внешние сервисы;
- отображает PDF, Office, legacy Office, CSV/TSV, SVG и популярные raster images;
- поддерживает документы с Latin/Cyrillic, CJK, Arabic-script и основными Indic scripts;
- предоставляет headless API и опциональный basic UI;
- остаётся пригодным для закрытого коммерческого использования за счёт permissive runtime-зависимостей;
- работает в последних двух версиях Chrome, Edge, Firefox и Safari с baseline Safari 16.4.

## Зафиксированные решения

### Форматы

- PDF.
- DOCX/DOCM, XLSX/XLSM, PPTX/PPTM/PPSX.
- DOC, XLS, PPT.
- CSV/TSV и SVG.
- PNG, JPEG, WebP, GIF, BMP и TIFF.

Macro-enabled Office отображается как обычный OOXML-контент; VBA никогда не выполняется. Spreadsheet formulas показываются по сохранённым cached values без вычислительного движка.

### Runtime и архитектура

- Полностью browser-side runtime; server fallback отсутствует.
- Логическое Rust/WASM-ядро модульное: форматные модули загружаются лениво через единый внутренний `FormatAdapter`.
- Modern Office использует pinned `@silurus/ooxml`; legacy Office нормализуется через pinned `office_oxide` в OOXML и затем проходит тот же renderer.
- PDF использует урезанную WASM-сборку `pdf_oxide` только с необходимыми feature flags.
- Parsing выполняется в Web Worker. Rendering виртуализирован и ограничен видимыми страницами/тайлами.
- COOP/COEP, WASM threads, SIMD и OffscreenCanvas не являются обязательными; при отсутствии возможностей используется совместимый fallback.

### Лицензии и зависимости

- Разрешены MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib и Unicode-3.0; для font assets также разрешены OFL-1.1 и Apache-2.0.
- GPL, AGPL, LGPL и MPL runtime-код запрещён.
- Версии зависимостей фиксируются только после задачи 01; исходные кандидаты: `@silurus/ooxml@0.72.2`, `office_oxide@0.1.6`, `pdf_oxide@0.3.74`.
- Если кандидат не проходит license, WASM, fidelity, security или size gate, его нельзя заменять copyleft-библиотекой: используется минимальный permissive fork/patch либо задача остаётся незавершённой.

### Fonts и языки

- Порядок разрешения fonts: embedded document fonts, зарегистрированные приложением fonts, CSS/system fonts, затем ленивые Noto WOFF2 packs по Unicode ranges.
- Font packs поставляются как отдельные assets внутри npm-дистрибутива и не загружаются без необходимости.
- Поддерживаются режимы `auto`, `offline` и `custom resolver`, а также self-host через `assetBaseUrl`.
- UI содержит встроенные локали `en` и `ru`; приложение может передать собственный словарь.

### UI и взаимодействие

- Headless API является основным контрактом; basic UI подключается опцией.
- Basic UI включает toolbar, pan/zoom/fit, page/slide navigation, sheet tabs, thumbnails, text search, selection/copy, fullscreen и download исходного файла.
- Выделение охватывает текстовые ranges и spreadsheet cell ranges.
- UI обязан работать мышью и клавиатурой. Полноценная accessibility/WCAG-поддержка не входит в v1.

### Безопасность и приватность

- Телеметрия отсутствует.
- По умолчанию: входной файл до 100 MiB, суммарно распакованные Office-данные до 512 MiB, отдельный ZIP entry до 64 MiB; limits настраиваются вниз или вверх приложением.
- External OOXML relationships, scripts и активный контент SVG блокируются.
- Внешние ссылки проходят scheme sanitization и могут полностью перехватываться host application.
- Password-protected PDF и Office возвращают типизированную ошибку `encrypted-document`; password flow в v1 отсутствует.

## Публичный контракт v1

- `ViewerClient.create(options)` создаёт runtime и управляет общими assets/fonts.
- `client.createViewer(options)` создаёт `DocumentViewer`; без `container` viewer работает headless.
- `DocumentSource`: `string | URL | Blob | File | ArrayBuffer | Uint8Array`.
- Основные методы viewer: `load`, `close`, `destroy`, `panBy`, `setZoom`, `zoomIn`, `zoomOut`, `fitWidth`, `fitPage`, `goToPage`, `next`, `previous`, `setSheet`, `search`, `searchNext`, `searchPrevious`, `clearSearch`, `getSelection`, `selectText`, `selectCells`, `clearSelection`, `downloadOriginal`.
- Headless rendering: `renderPage`, `renderThumbnail`, `renderSheetViewport`, `getPageText`, `getDocumentInfo`.
- Стабильные error codes: `unsupported-format`, `invalid-file`, `encrypted-document`, `resource-limit`, `network-error`, `aborted`, `font-unavailable`, `render-failed`.
- Все page/sheet indices нулевые.

## Границы работ

### Что входит в v1

- Все форматы из согласованной format matrix.
- Practical fidelity для распространённых конструкций и явные warnings для деградаций.
- TypeScript declarations, ESM exports, worker/WASM/font assets и CSS variables.
- Vanilla и React integration examples.
- Golden corpus, fuzz/security tests, browser matrix, bundle/license reports.

### Что переносится дальше

- Редактирование документов и сохранение изменений.
- Annotations, comments и undo/redo.
- Print pipeline.
- Вычисление spreadsheet formulas и выполнение macros.
- Password-protected documents.
- Server-side conversion/fallback.
- Формальная WCAG/a11y-поддержка.
- ODT/ODS/ODP, RTF, Markdown/TXT, HEIC и другие форматы вне согласованной matrix.

## Общие правила для всех задач

- Следовать порядку `Feature → Tests → Docs`.
- Публичный API нельзя менять без type tests, changelog note и обновления API reference.
- Любая parser/render dependency проходит license audit, WASM smoke test и corpus validation.
- Untrusted input обрабатывается только с limits, cancellation и typed errors; panic и зависание считаются release blockers.
- Нельзя добавлять сетевой вызов по умолчанию, кроме загрузки указанного source URL и собственных font assets.
- `destroy()` обязан освобождать DOM listeners, workers, object URLs, image bitmaps и WASM handles.

## Порядок задач

### Задача 01. Foundation и qualification зависимостей

**Статус:** ✅ Выполнена 2026-07-16

Цель: создать workspace, зафиксировать контракты и доказать пригодность выбранных permissive backends.

Результат: зафиксированы `@silurus/ooxml@0.72.2`, `office_oxide@0.1.6` и `pdf_oxide@0.3.74`; Chromium-прототипы и license gate прошли. Baseline parser WASM после Binaryen: 9.22 MiB raw / 3.05 MiB Brotli суммарно, с раздельной lazy-загрузкой.

Подробности: [`./todo/01-foundation-dependency-qualification.md`](./todo/01-foundation-dependency-qualification.md)

### Задача 02. Runtime, worker protocol и безопасность

**Статус:** ✅ Выполнена 2026-07-16

Цель: реализовать общий runtime, adapter contract, format detection, cancellation, limits и typed errors.

Результат: добавлены `ViewerClient`/`DocumentViewer`, registry и worker adapter/RPC, content-first detection, source streaming, стабильные errors/warnings, cancellation и pre-allocation limits для input/OOXML ZIP/raster images.

Подробности: [`./todo/02-runtime-security.md`](./todo/02-runtime-security.md)

### Задача 03. Modern и legacy Office pipeline

**Статус:** ✅ Выполнена 2026-07-16

Цель: подключить OOXML rendering и legacy→OOXML normalization с единым поведением viewer.

Результат: встроенный Office adapter автоматически маршрутизирует modern/macro-enabled и legacy formats; DOC/XLS/PPT нормализуются в module worker через bytes-in/bytes-out WASM, а metadata, render и sanitized text/hyperlink maps унифицированы. Spreadsheet formulas не вычисляются, macro content не исполняется.

Подробности: [`./todo/03-office-pipeline.md`](./todo/03-office-pipeline.md)

### Задача 04. PDF, images, CSV и SVG

**Статус:** ✅ Выполнена 2026-07-16

Цель: завершить format matrix отдельными безопасными adapters.

Результат: PDF и multi-page TIFF вынесены в lazy WASM workers, browser-native raster formats используют bounded decode path, SVG санитизируется без внешних ресурсов, CSV/TSV парсится в worker как single-sheet raw-string model. Default registry покрывает полную v1 format matrix.

Подробности: [`./todo/04-pdf-images-csv-svg.md`](./todo/04-pdf-images-csv-svg.md)

### Задача 05. Viewport, interaction и публичный API

**Статус:** ✅ Выполнена 2026-07-16

Цель: реализовать виртуализированный viewport, pan/zoom, navigation, search и selection.

Результат: единый immutable API управляет всеми adapters; continuous/single viewport ограничивает DOM видимым диапазоном, поддерживает pan/wheel/pinch/keyboard, search и logical text/cell selection. Headless render/thumbnail/sheet-region methods, typed events, cancellation и exact-original download покрыты unit и Chromium E2E тестами.

Подробности: [`./todo/05-viewport-interactions-api.md`](./todo/05-viewport-interactions-api.md)

### Задача 06. Fonts, i18n и basic UI

**Статус:** ✅ Выполнена 2026-07-16

Цель: обеспечить multilingual rendering и готовый управляемый UI без привязки к framework.

Результат: добавлены управляемые auto/offline/custom font policies, локальные lazy Noto WOFF2 packs для полной согласованной script matrix, `en`/`ru` locale overrides и capability-driven basic UI с toolbar, search, thumbnails, sheet tabs, fullscreen и original download. Font/UI workflows проходят unit и Chromium corpus tests без third-party network.

Подробности: [`./todo/06-fonts-i18n-basic-ui.md`](./todo/06-fonts-i18n-basic-ui.md)

### Задача 07. Hardening, performance и browser compatibility

**Статус:** ✅ Выполнена 2026-07-16

Цель: закрыть security, fidelity, memory, performance, size и browser release gates.

Результат: добавлены resource/time budgets, priority render scheduler, JS/Rust fuzzing, adversarial corpus, SSIM goldens, Chromium/Firefox/WebKit и fallback E2E, performance/lifecycle gates, полный size report и license/vulnerability audits. Base code+WASM — 3,26 MiB Brotli; все автоматизированные stage gates проходят.

Подробности: [`./todo/07-hardening-performance.md`](./todo/07-hardening-performance.md)

### Задача 08. Packaging, документация и release

**Статус:** ✅ Реализация и alpha artifact завершены 2026-07-16; публикация не выполнялась

Цель: подготовить переносимый npm artifact, integration examples и контролируемый выпуск 1.0.

Результат: подготовлен clean `0.1.0-alpha.0` tarball со стабильными exports, assets, licenses/notices, SPDX SBOM и integrity hashes. Он проходит six-consumer pack gate (plain ESM/SSR, strict TS, esbuild, Vite, webpack, Next.js), Vanilla/React builds и полный release checklist. Публикация/tag/1.0 остаются внешним авторизованным действием после real-browser smoke и alpha/beta feedback.

Подробности: [`./todo/08-packaging-release.md`](./todo/08-packaging-release.md)

## Итоговые quality gates для promotion в 1.0

Автоматизированные gates ниже реализованы, кроме целевого расширения corpus: текущий проверенный alpha corpus содержит семь pinned representative files плюс generated/adversarial fixtures. До promotion в 1.0 он должен быть расширен до указанного объёма и дополнен результатами real-browser/device smoke и alpha/beta feedback.

- Format corpus содержит не менее 20 representative files на каждое семейство форматов плюс corrupted/adversarial fixtures.
- Для фиксированного font corpus SSIM не ниже `0.97` для PDF/images, `0.94` для modern Office и `0.90` для legacy conversion; отдельно проверяется сохранность текста, изображений, tables, slides и sheets.
- Language corpus покрывает Latin/Cyrillic, zh-Hans/zh-Hant, Japanese, Korean, Arabic/Persian/Urdu и Devanagari, Bengali, Gujarati, Gurmukhi, Odia, Tamil, Telugu, Kannada, Malayalam.
- Pan/zoom не создаёт long tasks свыше 50 ms на эталонном сценарии; первая видимая страница 10 MiB fixture появляется не позднее 2.5 s на зафиксированном benchmark host.
- Целевой JS+WASM budget — 20 MiB Brotli без font packs; 20–25 MiB требует опубликованного size report, свыше 25 MiB блокирует release до отдельного согласования.
- Cargo/TypeScript tests, WASM/browser tests, license/vulnerability audits, golden diff и integration examples проходят без исключений.

## Ожидаемый итог

После завершения feature-пакета проект имеет production-ready browser document viewer с единым TypeScript API, permissive Rust/WASM backends, полной согласованной format matrix, multilingual rendering, basic UI, воспроизводимыми quality gates и документацией для интеграции в обычные web applications.
