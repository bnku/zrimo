# Задача 14. Cross-format requalification и новый alpha

**Статус:** 🟠 Requalification выполнена 2026-07-17; alpha promotion заблокирован задачей 13

## Цель

Повторно квалифицировать исправленный viewer на содержательных oracles и только
после этого собрать новый clean alpha artifact.

## Источники требований

- [`../01-fidelity-corrective-roadmap.md`](../01-fidelity-corrective-roadmap.md)
- Задачи 09–13.

## Зависимости

- Задачи 09–13 завершены либо format matrix изменена отдельным согласованным
  product decision.

## Что входит в задачу

- Полный cross-format fidelity/browser/security/performance rerun.
- Clean pack, SBOM/notices/size reports и examples.
- Новый alpha version; beta/1.0 остаются отдельным promotion decision.

## Что НЕ делаем в этой задаче

- Не ослабляем gates ради зелёного отчёта.
- Не включаем private regression inputs/outputs в artifact или reports.
- Не выполняем external npm publish/tag без явной авторизации.

## Основные задачи по слоям

### Feature

- Удалить superseded workers/WASM/assets и dead compatibility paths.
- Проверить shared API для page/slide/sheet layouts, selection и asset resolver.
- Обновить version/changelog только после зелёной qualification matrix.

### Tests

- Пройти задачи 10–13 corpus gates во всех supported browsers и offline mode.
- Пройти existing unit/type/Rust/WASM/fuzz/security/lifecycle/performance tests.
- Проверить representative formats минимум по конструкциям, а не только по
  extension/open success; отдельно зафиксировать unsupported/degraded features.
- Собрать tarball из clean checkout, распаковать и запустить consumer matrix.
- Проверить tarball allowlist, dependency licenses, SBOM, integrity, raw/Brotli
  size и отсутствие private/temp/corpus content.
- Запустить Vanilla и React dev/build smoke с реальным packaged dependency.

### Docs

- Исправить architecture/capability claims по фактическим backend.
- Опубликовать sanitized fidelity, browser и size reports.
- Обновить release checklist и migration notes; не объявлять production-ready до
  alpha/beta feedback и отдельного promotion.

## Критерии готовности

- Все corrective gates зелёные на clean checkout и packed consumers.
- Reports не содержат private filenames, hashes, text или images.
- Package содержит только allowlisted runtime/docs/license assets.
- Новый alpha готов локально; publish/tag не выполнялись.

## Результат

Локальная сборка `0.1.0-alpha.1` прошла TypeScript/Rust checks, 68 unit tests,
qualification DOC-refusal + XLS/PPT conversion, Chromium E2E, browser matrix
(31 pass, один заранее объявленный DPR2 skip), PDF font corpus, DOCX selection,
large-sheet virtualization, JS/Rust fuzz, license и vulnerability gates.
Base package занимает 3.51 MiB Brotli; со всеми optional fonts — 13.96 MiB.
Временный pack содержит 307 allowlisted files, проходит recursive content scan
и шесть clean consumer builds. Vanilla и React production builds проходят.

Однако corrective matrix намеренно оставляет `legacy-doc-structured-layout` в
`unsupported`: spike задачи 13 дал `no-go`, а отдельного product decision снять
DOC из promised scope не было. Поэтому `release-status.json` остаётся `blocked`,
pack report помечен `releaseCandidate: false`, а publish/tag не выполнялись.
Sanitized сводка: [`../../testing/qualification-2026-07-17.md`](../../testing/qualification-2026-07-17.md).
