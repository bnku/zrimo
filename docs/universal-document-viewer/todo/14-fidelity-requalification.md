# Задача 14. Cross-format requalification и стабильный 0.1.0

**Статус:** ✅ Завершена 2026-07-17; `0.1.0` готов локально, публикация не выполнялась

## Цель

Повторно квалифицировать исправленный viewer на содержательных oracles и только
после этого собрать clean stable artifact `0.1.0`.

## Источники требований

- [`../01-fidelity-corrective-roadmap.md`](../01-fidelity-corrective-roadmap.md)
- Задачи 09–13.

## Зависимости

- Задачи 09–13 завершены либо format matrix изменена отдельным согласованным
  product decision.

## Что входит в задачу

- Полный cross-format fidelity/browser/security/performance rerun.
- Clean pack, SBOM/notices/size reports и examples.
- Stable `0.1.0`; последующие minor/1.0 остаются отдельным promotion decision.

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
- Stable `0.1.0` готов локально; publish/tag не выполнялись.

## Результат

Локальная сборка `0.1.0` прошла TypeScript/Rust checks, 69 viewer и 68 Rust unit
tests, 7 public corpus qualification tests, 35 Chromium E2E и browser matrix
40/40 без skip на Chromium, Chromium DPR2, Firefox и WebKit. Fidelity matrix
включает modern Office, legacy DOC, PDF и image goldens. PDF font corpus, DOCX
selection, large-sheet virtualization, JS fuzz и пять Rust fuzz targets также
проходят.

Base runtime занимает 3,799,154 bytes Brotli; со всеми optional fonts —
14,759,868 bytes. Tarball содержит 308 allowlisted files, проходит recursive
content scan, asset-copy CLI, npm publish dry-run и шесть clean consumer builds.
`release-status.json` имеет состояние `ready`, pack report помечен
`releaseCandidate: true`. Publish, Git tag и GitHub Release не выполнялись.
Sanitized сводка: [`../../testing/qualification-0.1.0.md`](../../testing/qualification-0.1.0.md).
