# Задача 10. DOCX text layer и selection mapping

**Статус:** ✅ Завершена 2026-07-17

## Цель

Совместить selectable DOM с canvas-глифами DOCX и сделать drag/copy стабильными
на любом zoom, направлении текста и границе run/line.

## Источники требований

- [`../01-fidelity-corrective-roadmap.md`](../01-fidelity-corrective-roadmap.md)
- [`./05-viewport-interactions-api.md`](./05-viewport-interactions-api.md)

## Зависимости

- Задача 09 завершена.

## Что входит в задачу

- Полный text-run geometry contract.
- Format-specific DOCX text layer на базе `@silurus/ooxml`.
- Logical offset/cluster mapping и отдельный highlight overlay.
- Browser selection/copy tests на mixed scripts.

## Что НЕ делаем в этой задаче

- Не меняем визуальный DOCX canvas renderer.
- Не внедряем OCR.
- Не считаем programmatic selection достаточной заменой native drag selection.

## Основные задачи по слоям

### Feature

- Расширить внутренний `DocxRun`/`TextRun`: сохранить `fontSize`, CSS `font`,
  `letterSpacingPx`, `transform`, vertical/east-Asian flags и hyperlink metadata.
- Перестать строить DOCX spans общим упрощённым `#buildTextLayer`; вызвать
  upstream `buildDocxTextLayer` с теми же runs и page dimensions, что использовал
  canvas renderer.
- Ввести adapter-specific text-layer strategy, чтобы PDF/DOCX/PPTX не
  ограничивались наименьшим общим bounding-box contract.
- Хранить logical order отдельно от DOM order; сопоставлять DOM range с UTF-16
  public offsets и grapheme/cluster boundaries. Добавлять semantic separators
  между lines/paragraphs только из backend evidence.
- Рисовать find highlights отдельными inert boxes (`pointer-events:none`), не
  меняющими метрики selectable spans.

### Tests

- Geometry unit tests для font shorthand, scale, letter spacing, rotation,
  vertical text и hyperlink spans.
- Chromium/Firefox/WebKit drag tests: вперед/назад, внутри run, через runs/lines,
  double-click word, copy и programmatic selection.
- Zoom matrix `0.5`, `1`, `2`, DPR `1/2`, resize и repeated zoom settle.
- Script matrix: Latin+Cyrillic, CJK, Arabic/Bidi и Indic grapheme clusters.
- Visual/hit-test gate измеряет overlap selection rects с known glyph/run bounds;
  giant overlapping rectangles и выбор соседних строк блокируют merge.

### Docs

- Обновить text coordinate/selection contract в API и viewport architecture.
- Отметить различие UTF-16 public offsets и grapheme-aware UI boundaries.
- Документировать adapter-specific overlay lifecycle и cleanup.

## Критерии готовности

- Native selection визуально совпадает с glyphs на всей zoom/script matrix.
- Copy возвращает logical text без перестановки Bidi runs и без случайного
  слияния соседних слов/строк.
- Search highlights не влияют на selection hit testing.
- Старые public selection methods проходят type/behavior compatibility tests.

## Фактический результат

- `TextRun` расширен точным CSS `font`, `fontSize`, `letterSpacingPx`, renderer
  transform, tate-chu-yoko flag, natural coordinate extent и explicit logical
  UTF-16 offsets. DOCX adapter больше не теряет эти поля, сохраняет безопасные
  hyperlinks и выводит font family/weight/style для font policy.
- Page slot разделён на canvas, inert highlight layer и selectable text layer.
  DOCX лениво загружает `buildDocxTextLayer` и `buildDocxHighlightLayer` из
  pinned `@silurus/ooxml`; generic formats сохраняют отдельную fallback strategy.
  Natural overlay масштабируется одним transform, поэтому font metrics и pitch
  совпадают с canvas при zoom и DPR.
- Search match преобразуется в upstream run slices; highlight boxes имеют
  `pointer-events:none` и больше не меняют background selectable spans.
- Native Range endpoints переводятся в public UTF-16 offsets и защёлкиваются на
  grapheme boundaries с синхронной коррекцией DOM selection. Programmatic API
  сохраняет прежнюю точную UTF-16 семантику.
- Unit tests проверяют font shorthand, letter spacing, rotation, vertical flag,
  hyperlink и emoji/Indic cluster mapping. Browser suite проверяет ≥90% overlap,
  отсутствие giant rectangles, zoom `0.5/1/2`, resize, DPR `1/2`, mixed
  Latin/Cyrillic/CJK/Arabic/Indic, forward/backward pointer drag, double-click,
  logical copy и inert highlights.
- Selection suite проходит в Chromium, Firefox и WebKit; отдельный matrix
  project запускает Chromium с `deviceScaleFactor: 2`. Public API/headless tests
  остаются совместимыми.
