# Задача 07. Hardening, performance и browser compatibility

**Статус:** ✅ Выполнена 2026-07-16

## Цель

Превратить функционально полный viewer в release candidate: закрыть adversarial input, browser differences, fidelity regressions, memory leaks, responsiveness и размер production artifacts.

## Источники требований

- [`../00-roadmap.md`](../00-roadmap.md)
- [`./01-foundation-dependency-qualification.md`](./01-foundation-dependency-qualification.md)
- [`./02-runtime-security.md`](./02-runtime-security.md)
- [`./03-office-pipeline.md`](./03-office-pipeline.md)
- [`./04-pdf-images-csv-svg.md`](./04-pdf-images-csv-svg.md)
- [`./05-viewport-interactions-api.md`](./05-viewport-interactions-api.md)
- [`./06-fonts-i18n-basic-ui.md`](./06-fonts-i18n-basic-ui.md)

## Зависимости

- Задачи 01–06 функционально завершены.

## Что входит в задачу

- Rust fuzzing и malformed/adversarial corpus.
- Visual/structural fidelity gates по всей format/language matrix.
- Browser compatibility и fallback paths.
- Performance, memory, cache и size optimization.
- License/vulnerability audit как release gates.

## Что НЕ делаем в этой задаче

- Не расширяем format matrix и product scope.
- Не скрываем regression увеличением tolerances без отдельного документированного решения.
- Не включаем запрещённую библиотеку ради исправления одного browser/format case.
- Не оптимизируем benchmark ценой снятия security limits или cleanup.

## Основные задачи по слоям

### Feature

- Подключить fuzz targets для format detection, ZIP/CFB/XML parsing, legacy conversion, PDF parser, SVG sanitizer и CSV parser.
- Добавить time/iteration guards там, где malformed input способен вызвать pathological parsing/rendering.
- Настроить render queue priority для visible unit, соседних units и thumbnails; stale renders отменять.
- Ввести budgets для decoded pixels, canvas/tile cache, text maps и concurrent renders.
- Оптимизировать WASM через minimal features, LTO, `opt-level=z`/измеренный alternative, `wasm-opt` и asset compression.
- Проверить fallback без OffscreenCanvas, WebAssembly SIMD, `bitmaprenderer`, fullscreen API и modern observer features.
- Зафиксировать graceful low-memory behavior: cache eviction, warning и typed failure вместо tab crash.
- Сформировать machine-readable capability report по browser/format.

### Tests

- Запуск fuzzing с минимальным CI budget на каждый PR и расширенным nightly budget.
- Corrupted/truncated/encrypted/ZIP bomb/XML entity/SVG script/oversized image fixtures по каждой применимой ветке.
- Golden fidelity: SSIM `≥0.97` PDF/images, `≥0.94` modern Office, `≥0.90` legacy плюс structural assertions.
- Browser e2e: Playwright Chromium, Firefox, WebKit; release smoke на реальном Safari 16.4+ и последних двух версиях Chrome/Edge/Firefox/Safari.
- Performance benchmark: первая видимая страница 10 MiB fixture `≤2.5 s` на documented host; pan/zoom не создаёт long tasks `>50 ms` в эталонном сценарии.
- Large-document tests для тысячи pages/slides, large sheets и repeated zoom/search/selection.
- Memory tests repeated open/close и final destroy; worker termination должен освобождать WASM heap process resources.
- Bundle report raw/gzip/Brotli по каждому asset и по full code set; target 20 MiB, 20–25 MiB требует explanation, >25 MiB блокирует release.
- License and vulnerability audits выполняются на Rust, npm и bundled font/math assets.

### Docs

- Создать `docs/performance.md` с benchmark host, datasets, commands, results и tuning knobs.
- Создать `docs/compatibility.md` с browser/format matrix и known limitations.
- Обновить `docs/security.md` результатами fuzzing и audited limits.
- Публиковать bundle/license report как release artifact и кратко отражать regressions в changelog.

## Критерии готовности

- Все security, golden, browser, performance, memory, size и license gates автоматизированы или имеют документированный release smoke procedure.
- Нет известных panic, hang, uncontrolled allocation или active-content execution на corpus.
- Viewer остаётся управляемым на large documents благодаря virtualization/cancellation/cache eviction.
- Все supported browsers проходят основные load→navigate→zoom→search→select→destroy сценарии.
- Size находится в согласованном диапазоне либо имеет явно согласованный report; свыше 25 MiB release отсутствует.
- Capability и known-limitations документация соответствует фактическим тестам.

## Фактический результат

- Добавлены настраиваемые budgets для SVG, CSV cells, text maps, document units, render concurrency и времени операции. Таймаут завершает worker/WASM и возвращает `resource-limit`.
- Общий priority scheduler ограничивает параллелизм, обслуживает visible → adjacent → background и удаляет отменённые stale renders. Virtualization и PDF LRU cache остаются bounded на 1 000–10 000 units.
- JS mutation fuzz прошёл 2 000 детерминированных inputs без сбоев; libFuzzer targets `format_detection`, `legacy_office`, `pdf_parser` и `tiff_parser` прошли по 10 секунд без crash. Adversarial inventory покрывает ZIP/expanded bombs, encryption, oversized raster/SVG/CSV/text maps, active SVG и worker hang.
- Chromium golden/performance suite проходит 21/21. SSIM baselines дают 1,00 при gates 0,97 PDF/images, 0,94 modern Office и 0,90 legacy. 10 MiB runtime fixture показал 15,8 ms до первого render, 0 ms observed long task и два canvas при лимите пять на зафиксированном host.
- Playwright Chromium, Firefox и WebKit проходят load → navigate → zoom → search → select → destroy и fallback без OffscreenCanvas/ResizeObserver/createImageBitmap/bitmaprenderer. Capability JSON публикуется по каждому engine; real-browser Safari/Chrome/Edge/Firefox smoke остаётся обязательным шагом release checklist.
- Полный base code+WASM составляет 9,97 MiB raw / 4,30 MiB gzip / 3,26 MiB Brotli. Опциональные fonts исключены из 20 MiB gate и измеряются отдельно.
- npm production и RustSec audits нашли 0 vulnerabilities; license gate проверяет npm/Cargo expressions и byte length/SHA-256 всех 14 OFL font packs. Информационные RustSec notices по transitive `rustybuzz`/`ttf-parser` документированы и не являются vulnerability.
- Созданы `docs/performance.md`, `docs/compatibility.md`; обновлены security model и changelog. Machine reports лежат в `artifacts/`.
