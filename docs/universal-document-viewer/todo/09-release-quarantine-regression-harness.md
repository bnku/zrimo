# Задача 09. Release quarantine и regression harness

**Статус:** ✅ Завершена 2026-07-17

## Цель

Остановить promotion дефектного alpha, превратить четыре найденных класса ошибок
в воспроизводимые gates и гарантировать, что приватные диагностические файлы не
попадут ни в один release input/output.

## Источники требований

- [`../01-fidelity-corrective-roadmap.md`](../01-fidelity-corrective-roadmap.md)
- [`./07-hardening-performance.md`](./07-hardening-performance.md)
- [`./08-packaging-release.md`](./08-packaging-release.md)

## Зависимости

- Нет; это первый corrective stage.

## Что входит в задачу

- Release-blocked status и новая qualification matrix.
- Local-only private regression lane без persisted outputs.
- Независимые synthetic/provenanced committed fixtures.
- Tarball content guard и clean-checkout verification.

## Что НЕ делаем в этой задаче

- Не исправляем format renderers.
- Не копируем, не минимизируем и не публикуем приватные documents.
- Не обновляем release artifact до прохождения задачи 14.

## Основные задачи по слоям

### Feature

- Пометить текущий alpha как не прошедший fidelity qualification; запретить beta
  promotion в release script/checklist.
- Добавить opt-in runner, который читает private fixtures только из внешнего
  environment path, не перечисляет имена и пишет лишь обезличенный case result.
- Определить независимые synthetic fixtures: mixed-metric DOCX text, large XLSX
  used range, PDF font-family matrix и generated Word Binary table fixture либо
  permissive public DOC fixture с provenance manifest.
- Усилить pack scanner: allowlist package files, forbidden temp/corpus paths,
  recursive archive inspection и fail-closed поведение.

### Tests

- Pack test запускается с намеренно подложенным sentinel в ignored private path и
  доказывает его отсутствие в tarball и reports.
- Clean-checkout CI не зависит от private lane; private lane корректно skip-ается
  без environment path.
- Regression manifest хранит license/source/generator для каждого committed
  fixture и запрещает неизвестную provenance.
- Baseline tests сначала должны краснеть по четырём классам дефектов либо явно
  отмечать unsupported gate — зелёный smoke без проверяемого oracle запрещён.

### Docs

- Описать два test lanes, fixture provenance и правила обработки private input.
- Обновить release checklist: новый artifact возможен только после задачи 14.
- Зафиксировать, какие reports являются release inputs и как они санитизируются.

## Критерии готовности

- Текущий alpha нельзя ошибочно promote существующей командой/checklist.
- Private fixture lane воспроизводит дефекты локально, но не оставляет данных в
  workspace release scope.
- Synthetic/provenanced baseline включён в CI.
- Tarball scanner доказывает отсутствие private/temp/corpus content.

## Фактический результат

- `release-status.json` блокирует `alpha`, `beta` и `latest`; команда
  `npm run release:gate -- --channel <channel>` дополнительно проверяет version и
  отсутствие `unsupported` cases, поэтому одного ручного изменения статуса
  недостаточно для promotion.
- `tests/regressions/manifest.json` содержит четыре format-specific oracle с
  pinned Apache-2.0 provenance. Пока задачи 10–13 не предоставили executable
  oracle, cases честно имеют состояние `unsupported` и удерживают quarantine.
- `npm run test:regressions` входит в общий `check`, валидирует provenance и
  согласованность release status. Private lane включается только через
  `PRIVATE_REGRESSION_DIR`, допускает ignored `.tmp/` либо внешний каталог,
  выводит только case-id/result и не создаёт outputs.
- Pack test создаёт случайный sentinel в ignored private path, сравнивает
  `npm pack --dry-run` с реальным списком, применяет строгий package allowlist и
  рекурсивно распознаёт ZIP/GZIP/TAR и document/image signatures. Временный
  tarball и checksum остаются в `.cache/pack-test`; release artifact не меняется.
- Автотесты покрывают clean/private lane, отсутствие side effects, allowlist,
  sentinel и скрытый nested ZIP. Реальный tarball прошёл recursive scan для 111
  entries и установку в шесть clean consumer configurations.
- Политика lanes, provenance и sanitized reports описана в
  [`../../testing/regressions.md`](../../testing/regressions.md), release
  checklist обновлён.
