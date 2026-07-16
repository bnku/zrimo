# Задача 01. Foundation и qualification зависимостей

**Статус:** ✅ Выполнена 2026-07-16

## Цель

Создать воспроизводимую основу проекта и до production-разработки доказать, что выбранные permissive библиотеки действительно собираются в browser WASM, дают нужные данные для rendering/selection и укладываются в quality и size budgets.

Эта задача является обязательным readiness gate для всех форматных adapters. README или заявленная поддержка формата не считаются достаточным подтверждением.

## Источники требований

- [`../00-roadmap.md`](../00-roadmap.md)
- [`@silurus/ooxml`](https://github.com/yukiyokotani/office-open-xml-viewer)
- [`office_oxide`](https://github.com/yfedoseev/office_oxide)
- [`pdf_oxide`](https://github.com/yfedoseev/pdf_oxide)

## Зависимости

- Нет. Это первая задача feature-пакета.

## Что входит в задачу

- Cargo workspace и npm workspace для Rust crates, TypeScript package, examples и test tooling.
- ESM-first package skeleton `docs-viewer-wasm` с TypeScript declarations и SSR-safe imports.
- Exact-version/commit qualification для `@silurus/ooxml`, `office_oxide` и `pdf_oxide`.
- License allowlist и автоматический transitive dependency audit.
- Начальный golden/security/language corpus с правилами хранения provenance.
- WASM size, startup, memory и rendering smoke benchmarks.
- ADR по modular WASM architecture и границе Rust/TypeScript ответственности.

## Что НЕ делаем в этой задаче

- Не реализуем законченный viewer UI.
- Не доводим format adapters до production coverage.
- Не публикуем npm package.
- Не добавляем copyleft fallback, даже если он быстрее закрывает формат.

## Основные задачи по слоям

### Feature

- Настроить Node 22+, TypeScript, Node test runner, Playwright, Rust stable, `wasm-bindgen`, Binaryen, linting и formatting. Vitest/Vite исключены после license gate из-за MPL-транзитива.
- Создать логические workspace-модули для runtime core, format adapters, npm facade и examples без преждевременного копирования upstream code.
- Собрать browser smoke prototypes: OOXML load/render, legacy parse→IR/OOXML bytes, PDF render→bitmap/text map.
- Проверить, что для legacy conversion можно добавить минимальный bytes-out binding поверх `office_oxide`; зафиксировать upstream patch или точный fork commit.
- Зафиксировать внутренний `FormatAdapter` draft и общие типы `DocumentBackend`, `DocumentInfo`, render unit и text/cell map.
- Настроить `cargo-deny`/аналогичный license gate и JS license inventory с allowlist из roadmap.
- Создать benchmark manifest: reference hardware, sample sizes, commands и формат отчёта.

### Tests

- Проверить `wasm32-unknown-unknown` build каждого Rust-кандидата с минимальными feature flags.
- На representative fixtures подтвердить metadata, page/sheet/slide counts, bitmap output, text extraction и координаты selection map.
- Получить первые visual baselines и убедиться, что измерение SSIM/pixel diff воспроизводимо.
- Измерить raw, gzip и Brotli sizes каждого candidate artifact отдельно и совместно.
- Проверить отсутствие запрещённых лицензий во всех runtime dependency trees.
- Проверить импорт npm skeleton в browser, Node SSR import, Vite и Playwright smoke page.

### Docs

- Создать `docs/architecture.md` с modular runtime diagram и ownership границами.
- Создать `docs/dependencies.md` с pinned versions/commits, license, upstream URL, включёнными features и qualification result.
- Создать `docs/testing/corpus.md` с provenance, expected outputs и правилами добавления fixtures.
- Обновить roadmap фактически выбранными versions и размерными baseline.

## Критерии готовности

- Workspace собирается одной документированной командой в чистом окружении.
- Все выбранные runtime dependencies проходят permissive license gate.
- OOXML, legacy и PDF prototypes работают в browser без server runtime.
- Legacy bytes-out path технически доказан и не требует copyleft-кода.
- Зафиксированы version/commit pins, initial bundle report и corpus rules.
- Roadmap и следующие task-документы не содержат неизвестного выбора основной backend-библиотеки.

## Фактический результат

### Feature

- Созданы Cargo/npm workspaces, Rust crates `viewer-core`, `legacy-office-wasm`, `pdf-wasm`, ESM npm facade и vanilla example.
- Добавлены SSR-safe controller draft, `DocumentAdapter` и общие format/render/text/cell contracts.
- Реализованы in-memory legacy bytes-out binding и viewer-oriented PDF binding; matching `wasm-bindgen-cli` устанавливается локально, Binaryen закреплён в npm lockfile.
- Добавлены воспроизводимый corpus fetcher, SPDX allowlist, package/Cargo inventory и локальные browser binding scripts.

### Tests

- `wasm32-unknown-unknown` release build проходит для обоих project WASM adapters.
- Native corpus подтверждает DOC/XLS/PPT→OOXML и PDF PNG/positioned text; Chromium подтверждает DOCX rendering, legacy conversion и PDF render/text.
- Добавлен повторяемый DOCX screenshot baseline. Совокупный parser WASM: 9.22 MiB raw / 4.04 MiB gzip / 3.05 MiB Brotli.
- License inventory проходит для 61 npm lock entries и 192 Cargo packages; запрещённых runtime licenses нет.

### Docs

- Архитектура: [`../../architecture.md`](../../architecture.md).
- Pins и license decisions: [`../../dependencies.md`](../../dependencies.md).
- Corpus policy: [`../../testing/corpus.md`](../../testing/corpus.md).
- Qualification и size report: [`../../testing/qualification-2026-07-16.md`](../../testing/qualification-2026-07-16.md).
