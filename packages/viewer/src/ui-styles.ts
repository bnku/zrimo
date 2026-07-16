export const viewerCss = `
.docs-viewer-ui {
  --docs-viewer-background: #e9edf2;
  --docs-viewer-surface: #ffffff;
  --docs-viewer-text: #101828;
  --docs-viewer-muted: #667085;
  --docs-viewer-border: #d0d5dd;
  --docs-viewer-primary: #175cd3;
  --docs-viewer-primary-contrast: #ffffff;
  --docs-viewer-highlight: rgb(255 215 0 / 45%);
  --docs-viewer-radius: 6px;
  --docs-viewer-toolbar-height: 44px;
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 240px;
  overflow: hidden;
  color: var(--docs-viewer-text);
  background: var(--docs-viewer-background);
  font: 13px/1.4 "Docs Viewer Noto", system-ui, sans-serif;
}
.docs-viewer-ui, .docs-viewer-ui * { box-sizing: border-box; }
.docs-viewer-ui button, .docs-viewer-ui input {
  margin: 0;
  color: inherit;
  font: inherit;
}
.docs-viewer-ui__toolbar {
  z-index: 3;
  display: flex;
  flex: 0 0 var(--docs-viewer-toolbar-height);
  align-items: center;
  gap: 4px;
  min-width: 0;
  padding: 5px 8px;
  border-bottom: 1px solid var(--docs-viewer-border);
  background: var(--docs-viewer-surface);
}
.docs-viewer-ui__group { display: inline-flex; align-items: center; gap: 4px; }
.docs-viewer-ui__spacer { flex: 1 1 auto; }
.docs-viewer-ui__button {
  min-width: 32px;
  height: 32px;
  padding: 0 9px;
  border: 1px solid var(--docs-viewer-border);
  border-radius: var(--docs-viewer-radius);
  background: var(--docs-viewer-surface);
  cursor: pointer;
}
.docs-viewer-ui__button:hover { border-color: var(--docs-viewer-primary); }
.docs-viewer-ui__button[aria-pressed="true"] {
  color: var(--docs-viewer-primary-contrast);
  border-color: var(--docs-viewer-primary);
  background: var(--docs-viewer-primary);
}
.docs-viewer-ui__button:disabled { opacity: .45; cursor: default; }
.docs-viewer-ui__page-input {
  width: 54px;
  height: 30px;
  padding: 2px 5px;
  border: 1px solid var(--docs-viewer-border);
  border-radius: var(--docs-viewer-radius);
  text-align: center;
}
.docs-viewer-ui__body { position: relative; display: flex; flex: 1 1 auto; min-height: 0; }
.docs-viewer-ui__viewport { position: relative; flex: 1 1 auto; min-width: 0; min-height: 0; }
.docs-viewer-ui__panel {
  z-index: 2;
  flex: 0 0 220px;
  width: 220px;
  overflow: auto;
  border-right: 1px solid var(--docs-viewer-border);
  background: var(--docs-viewer-surface);
}
.docs-viewer-ui__panel[hidden], .docs-viewer-ui__search[hidden],
.docs-viewer-ui__sheets[hidden], .docs-viewer-ui [hidden] { display: none !important; }
.docs-viewer-ui__thumbnail {
  display: block;
  width: calc(100% - 20px);
  margin: 10px;
  padding: 6px;
  border: 1px solid var(--docs-viewer-border);
  border-radius: var(--docs-viewer-radius);
  background: var(--docs-viewer-surface);
  cursor: pointer;
}
.docs-viewer-ui__thumbnail[aria-current="page"] { border-color: var(--docs-viewer-primary); }
.docs-viewer-ui__thumbnail canvas { display: block; max-width: 100%; height: auto; margin: auto; }
.docs-viewer-ui__search {
  z-index: 4;
  display: flex;
  gap: 5px;
  align-items: center;
  padding: 7px 8px;
  border-bottom: 1px solid var(--docs-viewer-border);
  background: var(--docs-viewer-surface);
}
.docs-viewer-ui__search input {
  flex: 1 1 auto;
  min-width: 80px;
  height: 32px;
  padding: 5px 8px;
  border: 1px solid var(--docs-viewer-border);
  border-radius: var(--docs-viewer-radius);
}
.docs-viewer-ui__search-status { min-width: 90px; color: var(--docs-viewer-muted); }
.docs-viewer-ui__sheets {
  z-index: 3;
  display: flex;
  flex: 0 0 38px;
  align-items: center;
  gap: 4px;
  overflow-x: auto;
  padding: 4px 8px;
  border-top: 1px solid var(--docs-viewer-border);
  background: var(--docs-viewer-surface);
}
.docs-viewer-ui__status {
  position: absolute;
  z-index: 5;
  right: 10px;
  bottom: 10px;
  max-width: min(420px, calc(100% - 20px));
  padding: 6px 9px;
  border-radius: var(--docs-viewer-radius);
  color: var(--docs-viewer-primary-contrast);
  background: rgb(16 24 40 / 88%);
  pointer-events: none;
}
.docs-viewer-ui--fullscreen-fallback { position: fixed; z-index: 2147483647; inset: 0; }
@media (max-width: 640px) {
  .docs-viewer-ui__toolbar { overflow-x: auto; }
  .docs-viewer-ui__button { padding-inline: 7px; }
  .docs-viewer-ui__panel { position: absolute; inset: 0 auto 0 0; }
  .docs-viewer-ui__label { display: none; }
}
`;
