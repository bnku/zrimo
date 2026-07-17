export const viewerCss = `
.zrimo-ui {
  --zrimo-background: #e9edf2;
  --zrimo-surface: #ffffff;
  --zrimo-text: #101828;
  --zrimo-muted: #667085;
  --zrimo-border: #d0d5dd;
  --zrimo-primary: #175cd3;
  --zrimo-primary-contrast: #ffffff;
  --zrimo-highlight: rgb(255 215 0 / 45%);
  --zrimo-radius: 6px;
  --zrimo-toolbar-height: 44px;
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 240px;
  overflow: hidden;
  color: var(--zrimo-text);
  background: var(--zrimo-background);
  font: 13px/1.4 "Zrimo Noto", system-ui, sans-serif;
}
.zrimo-ui, .zrimo-ui * { box-sizing: border-box; }
.zrimo-ui button, .zrimo-ui input {
  margin: 0;
  color: inherit;
  font: inherit;
}
.zrimo-ui__toolbar {
  z-index: 3;
  display: flex;
  flex: 0 0 var(--zrimo-toolbar-height);
  align-items: center;
  gap: 4px;
  min-width: 0;
  padding: 5px 8px;
  border-bottom: 1px solid var(--zrimo-border);
  background: var(--zrimo-surface);
}
.zrimo-ui__group { display: inline-flex; align-items: center; gap: 4px; }
.zrimo-ui__spacer { flex: 1 1 auto; }
.zrimo-ui__button {
  min-width: 32px;
  height: 32px;
  padding: 0 9px;
  border: 1px solid var(--zrimo-border);
  border-radius: var(--zrimo-radius);
  background: var(--zrimo-surface);
  cursor: pointer;
}
.zrimo-ui__button:hover { border-color: var(--zrimo-primary); }
.zrimo-ui__button[aria-pressed="true"] {
  color: var(--zrimo-primary-contrast);
  border-color: var(--zrimo-primary);
  background: var(--zrimo-primary);
}
.zrimo-ui__button:disabled { opacity: .45; cursor: default; }
.zrimo-ui__page-input {
  width: 54px;
  height: 30px;
  padding: 2px 5px;
  border: 1px solid var(--zrimo-border);
  border-radius: var(--zrimo-radius);
  text-align: center;
}
.zrimo-ui__body { position: relative; display: flex; flex: 1 1 auto; min-height: 0; }
.zrimo-ui__viewport { position: relative; flex: 1 1 auto; min-width: 0; min-height: 0; }
.zrimo-ui__panel {
  z-index: 2;
  flex: 0 0 220px;
  width: 220px;
  overflow: auto;
  border-right: 1px solid var(--zrimo-border);
  background: var(--zrimo-surface);
}
.zrimo-ui__panel[hidden], .zrimo-ui__search[hidden],
.zrimo-ui__sheets[hidden], .zrimo-ui [hidden] { display: none !important; }
.zrimo-ui__thumbnail {
  display: block;
  width: calc(100% - 20px);
  margin: 10px;
  padding: 6px;
  border: 1px solid var(--zrimo-border);
  border-radius: var(--zrimo-radius);
  background: var(--zrimo-surface);
  cursor: pointer;
}
.zrimo-ui__thumbnail[aria-current="page"] { border-color: var(--zrimo-primary); }
.zrimo-ui__thumbnail canvas { display: block; max-width: 100%; height: auto; margin: auto; }
.zrimo-ui__search {
  z-index: 4;
  display: flex;
  gap: 5px;
  align-items: center;
  padding: 7px 8px;
  border-bottom: 1px solid var(--zrimo-border);
  background: var(--zrimo-surface);
}
.zrimo-ui__search input {
  flex: 1 1 auto;
  min-width: 80px;
  height: 32px;
  padding: 5px 8px;
  border: 1px solid var(--zrimo-border);
  border-radius: var(--zrimo-radius);
}
.zrimo-ui__search-status { min-width: 90px; color: var(--zrimo-muted); }
.zrimo-ui__sheets {
  z-index: 3;
  display: flex;
  flex: 0 0 38px;
  align-items: center;
  gap: 4px;
  overflow-x: auto;
  padding: 4px 8px;
  border-top: 1px solid var(--zrimo-border);
  background: var(--zrimo-surface);
}
.zrimo-ui__status {
  position: absolute;
  z-index: 5;
  right: 10px;
  bottom: 10px;
  max-width: min(420px, calc(100% - 20px));
  padding: 6px 9px;
  border-radius: var(--zrimo-radius);
  color: var(--zrimo-primary-contrast);
  background: rgb(16 24 40 / 88%);
  pointer-events: none;
}
.zrimo-ui--fullscreen-fallback { position: fixed; z-index: 2147483647; inset: 0; }
@media (max-width: 640px) {
  .zrimo-ui__toolbar { overflow-x: auto; }
  .zrimo-ui__button { padding-inline: 7px; }
  .zrimo-ui__panel { position: absolute; inset: 0 auto 0 0; }
  .zrimo-ui__label { display: none; }
}
`;
