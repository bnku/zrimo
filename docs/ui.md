# Optional basic UI

The basic UI is enabled with `ui: true` and a real container. Omitting `ui`, or omitting the container entirely, keeps the stable headless/controller API with no toolbar DOM.

```ts
const client = ViewerClient.create({
  assetBaseUrl: new URL("/zrimo/", location.href),
});
const viewer = client.createViewer({
  container: document.querySelector<HTMLElement>("#viewer")!,
  ui: true,
  locale: "ru",
  fit: "width",
});
await viewer.load(file, { fileName: file.name });
```

Give the container a defined height. The UI fills it and owns only children under `.zrimo-ui`. It injects scoped critical CSS for a no-setup default; the same stylesheet is exported as `@zrimo/viewer/styles.css` for CSP/build pipelines that prefer an explicit asset.

## Controls

The toolbar contains previous/next, zero-based-API/one-based-display page indicator, zoom in/out, fit width/page, search, thumbnails, fullscreen, and exact-original download. The UI reads the same immutable state/events as external code and does not maintain a competing document state.

Controls are capability-driven:

- single-page images hide page navigation and thumbnails;
- sheet documents show sheet tabs and the current `R…C…` selection range, while hiding page thumbnails;
- search is disabled without a backend logical text map;
- thumbnail DOM is bounded to an 11-page window around the current page.

Fullscreen uses the browser Fullscreen API and falls back to a fixed in-page overlay if the API is unavailable or denied.

## Keyboard and pointer behavior

| Input                                 | Action                                                   |
| ------------------------------------- | -------------------------------------------------------- |
| Pointer drag on canvas/background     | Pan                                                      |
| Ctrl/Cmd + wheel or two-pointer pinch | Zoom preview, then crisp render                          |
| Arrow keys                            | Pan                                                      |
| Page Up / Page Down                   | Viewport page step; in single layout, previous/next unit |
| Ctrl/Cmd + `+` / `-`                  | Zoom in/out                                              |
| Ctrl/Cmd + `F`                        | Open/focus search                                        |
| Enter / Shift+Enter in search         | Next/previous match                                      |
| Ctrl/Cmd + `C`                        | Copy logical selection                                   |
| F11                                   | Toggle fullscreen/fallback                               |
| Escape                                | Close panels and fallback fullscreen                     |
| Spreadsheet drag                      | Select cell range                                        |
| Shift+click in a spreadsheet          | Extend the active range from its anchor                  |
| Ctrl/Cmd+click or drag                | Toggle a cell or add a non-contiguous range              |
| Drag a spreadsheet column border      | Resize that column for the current viewing session       |
| Shift + arrows after a cell click     | Extend cell range                                        |

Viewer shortcuts are not intercepted while typing in an input, textarea, or editable host.

## Locales and overrides

Built-in dictionaries are `en` and `ru`. Resolution is requested locale → English → host overrides.

```ts
const viewer = client.createViewer({
  container,
  ui: true,
  locale: "ru",
  translations: {
    download: "Сохранить исходник",
    searchPlaceholder: "Поиск по файлу",
  },
});
```

`ViewerTranslations` is exported so an integration can type-check a complete external dictionary. UI content locale does not alter Unicode document search semantics.

## CSS variables

Override variables on the host or `.zrimo-ui` root:

```css
#viewer {
  --zrimo-background: #111827;
  --zrimo-surface: #1f2937;
  --zrimo-text: #f9fafb;
  --zrimo-muted: #9ca3af;
  --zrimo-border: #374151;
  --zrimo-primary: #7c3aed;
  --zrimo-primary-contrast: #fff;
  --zrimo-highlight: rgb(250 204 21 / 45%);
  --zrimo-radius: 8px;
  --zrimo-toolbar-height: 46px;
}
```

All component selectors start with `.zrimo-ui`; no generic `button`, `canvas`, or host selectors escape that scope. `destroy()` removes the viewport, toolbar, injected style, panels, timers, listeners, and pending thumbnail renders.

## React integration snippet

```tsx
import { useEffect, useRef } from "react";
import { ViewerClient } from "@zrimo/viewer";

export function DocumentPreview({ file }: { file: File }) {
  const container = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!container.current) return;
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/zrimo/", location.href),
    });
    const viewer = client.createViewer({
      container: container.current,
      ui: true,
      fit: "width",
    });
    void viewer.load(file, { fileName: file.name });
    return () => void client.destroy();
  }, [file]);

  return <div ref={container} style={{ height: 720 }} />;
}
```

For headless React integration, omit `container`/`ui` and keep the same lifecycle in the effect.
