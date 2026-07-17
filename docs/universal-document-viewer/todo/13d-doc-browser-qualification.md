# Задача 13d. DOC browser integration и qualification

**Статус:** 🟠 В работе с 2026-07-17; browser/pack gates проходят, visual matrix не завершён

## Цель

Включить faithful DOC path в npm package и доказать его browser, fidelity,
security, performance и packaging gates.

## Источники требований

- [`./13-legacy-doc-fidelity.md`](./13-legacy-doc-fidelity.md)
- [`./13c-doc-tables-media-serialization.md`](./13c-doc-tables-media-serialization.md)
- [`./14-fidelity-requalification.md`](./14-fidelity-requalification.md)

## Что входит

### Feature

- `legacy-doc` → `legacy-office-wasm` bytes-in/bytes-out binding в module worker.
- Cancellation/time/resource limits и transferable buffers без main-thread parse.
- Снять `fidelity-unsupported` только для квалифицированных DOC revisions.
- Сохранить diagnostic plain-text API отдельно от render path.
- Вернуть DOC в format capability matrix и release gate.

### Tests

- Browser E2E: open/render/search/select/zoom/pan/destroy для DOC corpus.
- Structural gates: sections, paragraphs/runs, table grids, cell order, styles,
  images и headers/footers; plain-text equality недостаточна.
- Visual SSIM gate `≥0.90` плюс hard assertions на tables и отсутствие fabricated
  headings/duplicate titles.
- WASM fuzz, cancellation, memory/time limits, repeated lifecycle и browser matrix.
- Clean pack/consumer test доказывает наличие нужных WASM assets и отсутствие
  private/temp inputs.

### Docs

- Обновить public Office feature matrix, warnings/errors и browser integration.
- Обновить SBOM, third-party notices, size report и release status.

## Что не делаем

- Не объявляем неподдержанные Word revisions или OLE features рабочими.
- Не ослабляем fail-closed path ради частичного текста.

## Критерии готовности

- Все DOC structural/visual/security/browser gates проходят в clean checkout.
- Npm tarball остаётся permissive и в size budget.
- Release blocker снимается только после повторной задачи 14 qualification.

## Текущий результат

- Старый TypeScript `fidelity-unsupported` gate снят: DOC вызывает bounded
  `legacy-doc` conversion в module worker и затем штатный DOCX renderer.
- Chromium E2E на публичных `word97-simple-table.doc` и
  `word97-comments.doc` проверяет WASM ZIP output, package adapter, non-empty
  text map, реальный canvas render, search/select/copy/zoom/pan/destroy и
  point-comment conversion и реальную `PlfLst`/`PlfLfo` numbering projection.
  Отдельный worker regression проверяет cancellation,
  `maxInputBytes` и успешную повторную загрузку после обеих ошибок.
- React dev integration отдельно проверена через file input; default
  self-hosted module URL исправлен на npm layout `assets/legacy/index.js`.
- Viewer unit tests, Rust workspace tests, strict clippy, WASM build, public
  Word 97/2000/2003 qualification, полный Chromium E2E и шесть clean
  pack/consumer gates проходят. E2E перед запуском обновляет static example
  directory, поэтому повторно используемый dev-сервер не может скрыто раздавать
  устаревший WASM.
- Native и Chromium corpus дополнены Apache-2.0 ranged-comment fixture:
  annotation bookmark проходит полный browser WASM path, а содержащиеся в DOC
  `NilPICFAndBinData` записи строго отделяются от PICF-картинок по `sprmCFData`.
- Ещё нужны cross-browser DOC matrix, visual oracle/golden и повторная задача
  14.
