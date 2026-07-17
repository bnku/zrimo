# Задача 08. Packaging, документация и release

**Статус:** ✅ Packaging завершён 2026-07-16; ⚠️ alpha artifact quarantined 2026-07-17

## Цель

Подготовить проверяемый npm artifact, который без скрытых шагов интегрируется в распространённые web toolchains, и провести контролируемый выпуск от alpha до production 1.0.

## Источники требований

- [`../00-roadmap.md`](../00-roadmap.md)
- Все задачи 01–07 этого feature-пакета.

## Зависимости

- Задачи 01–07 завершены; public API и release gates заморожены для v1.

## Что входит в задачу

- Production ESM package, declarations, CSS, worker, WASM и font assets.
- Stable exports и asset URL/self-host behavior.
- Integration verification для Vite, webpack/Next.js, Angular/esbuild и plain ESM.
- Vanilla и React examples.
- README, API guide, format/browser/security/font documentation и changelog.
- Alpha, beta и 1.0 release procedure.

## Что НЕ делаем в этой задаче

- Не добавляем framework-specific runtime dependencies.
- Не меняем public API после release candidate без возврата к tests/docs соответствующей задачи.
- Не публикуем package при незакрытом security/license/size gate.
- Не обещаем неподтверждённые форматы или fidelity в README.

## Основные задачи по слоям

### Feature

- Настроить package exports `.`, `/headless`, `/worker` и `/styles.css`, `types`, `files`, side effects и supported Node tooling metadata.
- Обеспечить asset resolution через `new URL(..., import.meta.url)` и override `assetBaseUrl`; документировать copy step только для bundlers, которые не обрабатывают WASM assets.
- Проверить, что package import безопасен в SSR/Node, а browser-only initialization происходит только при `create()`.
- Добавить reproducible build, integrity hashes/SBOM, third-party notices и provenance опубликованных artifacts.
- Создать vanilla example с basic UI и React example с корректным async lifecycle/cleanup.
- Зафиксировать semver policy, deprecation policy и changelog format.
- Настроить prerelease channels `alpha` и `beta`, затем immutable 1.0 tag после прохождения release checklist.

### Tests

- `npm pack` content test: присутствуют только нужные JS/types/CSS/WASM/worker/font/license files; отсутствуют source corpus и development secrets.
- Install tarball into clean fixture apps: Vite, webpack/Next.js, Angular/esbuild и plain ESM static server.
- Проверить base URL/CDN/self-host deployment, CSP-compatible worker strategy и MIME types для `.wasm`/fonts.
- Run examples against packed tarball, а не workspace source.
- TypeScript consumer tests с strict mode и declaration resolution всех exports.
- Release checklist повторно запускает unit/integration/e2e, golden, security, license, size и vulnerability gates.
- Verify download original, offline font mode и no-telemetry/no-unexpected-network behavior в packed build.

### Docs

- Создать root README с install, quick start, format matrix, browser support и privacy statement.
- Завершить API reference, headless/basic UI guides, fonts/self-host guide, security model и troubleshooting.
- Добавить framework integration recipes для tested toolchains и отдельные runnable examples.
- Создать `CHANGELOG.md`, `LICENSE`, `THIRD_PARTY_NOTICES`, `SECURITY.md` и release checklist.
- Обновить roadmap фактическими results; только после релиза перенести выполненные task documents в `done/` с сохранением ссылок.

## Критерии готовности

- Clean consumers устанавливают packed tarball и открывают representative PDF, modern Office, legacy Office и image без доступа к workspace.
- Все public exports и asset paths работают в согласованных toolchains.
- Package содержит полные notices/SBOM и не содержит запрещённых лицензий.
- README и compatibility matrix не расходятся с automated capability report.
- Alpha и beta feedback не оставляют unresolved release-blocking defects.
- Release 1.0 опубликован только после прохождения полного checklist и фиксации changelog/tag.

## Фактический результат

> Коррекция: последующая fidelity qualification выявила release blockers,
> поэтому описанный ниже artifact остаётся историческим packaging result и не
> является кандидатом на promotion. Актуальные gates определены в
> [`../01-fidelity-corrective-roadmap.md`](../01-fidelity-corrective-roadmap.md).

- Production ESM package экспортирует `.`, `/headless`, `/worker`, `/styles.css`, `/fonts/*`, `/workers/*` и `/assets/*`; declarations, workers, WASM, CSS и fonts входят в clean reproducible `dist` без source maps/corpus/source files.
- `npm pack` создал `@docs-viewer-wasm/viewer@0.1.0-alpha.0`: 111 файлов, 14 350 554 bytes compressed / 18 923 751 bytes unpacked. SHA-1/npm integrity и SHA-256 `b1832fc60ff93ad36ac1e7805db38ce960f24e6388b204de4343a21bc1596acf` сохранены рядом с tarball; forbidden-content scan прошёл.
- Tarball установлен в clean consumer и прошёл plain ESM/SSR import, strict TypeScript declaration resolution, Angular-style esbuild, Vite, webpack 5 и Next.js webpack production builds.
- Добавлены runnable Vanilla basic UI и React 19/Vite examples с корректным async destroy; root/package README, integrations, troubleshooting, compatibility, performance, security, fonts/UI/API docs и changelog согласованы.
- Добавлены dual MIT/Apache-2.0 licenses, third-party notices, проверяемый font OFL manifest, SPDX 2.3 SBOM на 288 third-party package records, `SHA256SUMS` и machine-readable pack/size/audit/browser/fidelity/performance/fuzz reports.
- CI выполняет build/check, JS fuzz, Rust fuzz smoke/nightly, audits, size/SBOM/pack gates и Chromium/Firefox/WebKit suites. Release checklist фиксирует alpha → beta → 1.0, semver/deprecation, CSP/MIME/self-host и real-browser smoke.
- Внешняя публикация в npm, alpha/beta feedback и git tag намеренно не выполнялись: для этого нужны registry scope/credentials, authorized maintainer и реальный Safari/Chrome/Edge/Firefox release smoke. Подготовленный immutable tarball является результатом этого этапа; он не выдаётся за опубликованный 1.0.
