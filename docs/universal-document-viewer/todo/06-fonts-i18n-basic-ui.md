# Задача 06. Fonts, i18n и basic UI

**Статус:** ✅ Выполнена 2026-07-16

## Результат

Добавлен shared `FontManager` с порядком adapter-embedded → app-registered → CSS/system → packaged fallback, политиками `auto`/`offline`/`custom`, host resolver, cache reuse, управляемым `FontFace` lifecycle и non-fatal `font-unavailable`. В npm assets включены 14 локальных OFL-1.1 WOFF2 packs из pinned Noto commits с Unicode ranges, manifest/SHA-256 и third-party notices; скрытых CDN-запросов нет.

Реализованы `en`/`ru` dictionaries и typed overrides. Опциональный `.zrimo-ui` содержит toolbar, page/zoom/fit controls, search, bounded thumbnails, sheet tabs/range, fullscreen fallback и exact-original download; controls следуют capabilities и синхронизируются только через public state/events. Pointer/keyboard workflows и scoped CSS variables документированы.

Проверки: 59 TypeScript tests и 13 Chromium E2E проходят. Browser corpus покрывает Latin/Cyrillic, zh-Hans/zh-Hant, Japanese, Korean, Arabic/Persian/Urdu и девять Indic scripts без missing font packs; отдельные tests фиксируют offline/no-fetch, custom/registered/system resolution, 404/corrupt fonts, cache, UI localization, CSS isolation, sheets, search, thumbnails, fullscreen и download.

## Цель

Обеспечить корректное отображение multilingual document content и поставить готовый, но опциональный basic UI, которым можно управлять мышью, клавиатурой и через public API.

## Источники требований

- [`../00-roadmap.md`](../00-roadmap.md)
- [`./05-viewport-interactions-api.md`](./05-viewport-interactions-api.md)

## Зависимости

- Задача 05 завершена; headless API и state/events стабильны.

## Что входит в задачу

- Font discovery/resolution, registration, fallback и lazy font packs.
- Проверяемая поддержка Latin/Cyrillic, CJK, Arabic-script и major Indic scripts.
- UI locale infrastructure с `en` и `ru`.
- Basic toolbar, thumbnails, search panel, sheet tabs, fullscreen и download.
- CSS variables и scoped styles без зависимости от UI framework.

## Что НЕ делаем в этой задаче

- Не заявляем WCAG compliance и не строим полный screen-reader document tree.
- Не включаем proprietary Microsoft fonts.
- Не делаем third-party CDN обязательным.
- Не добавляем annotations, outline/bookmarks panels, print dialog или themes beyond CSS variables.

## Основные задачи по слоям

### Feature

- Реализовать font order: embedded fonts → app-registered fonts → CSS/system fonts → packaged fallback packs.
- Определить `FontPolicy` с modes `auto`, `offline`, `custom`; `custom` получает family/weight/style/script/codepoints и возвращает font bytes/URL.
- Разбить Noto fallbacks на WOFF2 Unicode-range assets, чтобы документ загружал только встреченные ranges.
- Разрешать self-host font/WASM/worker assets через единый `assetBaseUrl`; все object URLs и FontFace registrations иметь управляемый lifecycle.
- Отдавать `font-unavailable` warning и список substitutions, но не блокировать документ, если доступен fallback glyph.
- Реализовать locale dictionaries, `locale`, `translations` override и fallback `requested → en`.
- Собрать basic toolbar: previous/next, page indicator, zoom in/out, fit width/page, search toggle, thumbnails toggle, fullscreen и download.
- Для spreadsheet показывать sheet tabs и текущий range; для image single-page скрывать нерелевантные controls через capabilities.
- Синхронизировать UI с external API/events без отдельного источника state.
- Реализовать keyboard shortcuts для navigation, zoom, search, copy и fullscreen; не перехватывать ввод в text fields.
- Изолировать styles под корневым class и предоставить документированный набор CSS custom properties.

### Tests

- Golden language fixtures для Latin/Cyrillic, zh-Hans/zh-Hant, Japanese, Korean, Arabic/Persian/Urdu и всех согласованных Indic scripts.
- Mixed-script tests для font fallback внутри одного run, RTL/LTR bidi, Arabic joining, Indic shaping и CJK line wrapping.
- Font policy tests embedded/system/pack/custom/offline, missing URL, corrupted font и cache reuse.
- Network tests подтверждают отсутствие third-party fetch и загрузку только реально нужных packaged ranges.
- UI component tests capability-driven controls, en/ru labels, translation override и error/progress states.
- Playwright tests mouse/keyboard workflows, fullscreen fallback, sheet tabs, thumbnails, search и download original.
- CSS isolation test подтверждает отсутствие style leakage в host application и обратно для критичных layout rules.

### Docs

- Создать `docs/fonts.md` с resolver contract, self-host/offline recipes, licensing и troubleshooting substitutions.
- Создать `docs/ui.md` с controls, shortcuts, locale extension и CSS variables.
- Добавить vanilla и React snippets для headless и basic UI modes.
- Создать third-party font notices и manifest с license/provenance каждого asset.

## Критерии готовности

- Language corpus проходит golden/structure checks без missing glyphs.
- Default integration работает без proprietary fonts и без third-party CDN.
- Offline/custom font modes не выполняют скрытых network calls.
- Basic UI содержит весь согласованный practical viewer scope и может быть полностью отключён.
- Mouse и keyboard workflows проходят e2e; WCAG не заявляется.
- UI strings, styles и font APIs документированы.
