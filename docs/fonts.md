# Fonts and multilingual fallback

The viewer never requires proprietary Microsoft fonts or a third-party font CDN. Embedded fonts remain an adapter concern and take precedence inside the source format. The shared browser runtime then resolves app-registered faces, explicitly requested CSS/system faces, and finally local packaged Noto WOFF2 ranges.

## Policy

```ts
const client = ViewerClient.create({
  assetBaseUrl: new URL("/zrimo/", location.href),
  fontPolicy: { mode: "auto" },
  fonts: [
    {
      family: "Corporate Sans",
      source: new URL("/brand/corporate-regular.woff2", location.href),
      weight: 400,
      style: "normal",
      scripts: ["latin", "cyrillic"],
    },
  ],
});
```

`FontPolicy.mode` has three values:

- `auto` (default) uses registered/system fonts and lazily fetches a matching packaged range under `assetBaseUrl/fonts/` when a deterministic fallback is needed.
- `offline` never fetches a fallback. Supply font bytes in `fonts` or ensure the required system faces exist. Missing coverage emits `font-unavailable` but does not abort document rendering.
- `custom` calls only the host resolver after registered/system resolution. Returning a URL is an explicit authorization to fetch that URL through the client's `fetch` hook.

## Custom resolver

```ts
const client = ViewerClient.create({
  fontPolicy: {
    mode: "custom",
    async resolver(request, signal) {
      // request: family?, weight, style, script, unique codepoints
      const match = await myFontCatalog.find(request, signal);
      return match ? { family: match.family, source: match.woff2Bytes } : null;
    },
  },
});
```

`source` may be `ArrayBuffer`, `Uint8Array`, `URL`, or a URL string. Bytes avoid all font network access. Loads are cached by face/script/style, concurrent requests share one promise, and corrupt/404 responses become non-fatal warnings. `client.destroy()` removes every managed `FontFace` from `document.fonts` and releases cached references.

## Packaged ranges and self-hosting

The npm artifact contains local WOFF2 packs for Latin/Cyrillic, Han, Japanese kana, Hangul, Arabic, Devanagari, Bengali, Gujarati, Gurmukhi, Odia, Tamil, Telugu, Kannada, and Malayalam. Only scripts discovered in the backend logical text map are requested. For example, Arabic plus Devanagari loads exactly:

```text
<assetBaseUrl>/fonts/noto-sans-arabic.woff2
<assetBaseUrl>/fonts/noto-sans-devanagari.woff2
```

Copy the complete `dist` assets to the URL represented by `assetBaseUrl`; workers, WASM, and fonts share that root. Serve `.woff2` as `font/woff2` and allow it in `font-src` CSP. No request is made to Google Fonts or another CDN.

The files are subsets from pinned official Noto commits, licensed under OFL-1.1. [The manifest](https://github.com/bnku/zrimo/blob/main/packages/viewer/fonts/manifest.json) records byte sizes and SHA-256 hashes; [third-party notices](https://github.com/bnku/zrimo/blob/main/packages/viewer/fonts/THIRD_PARTY_NOTICES.md) and the license ship beside the fonts. Font packs are lazy assets and excluded from the base JS+WASM transfer budget.

## Troubleshooting

- Listen for `warning` and inspect `warning.code === "font-unavailable"` or `"font-substitution"`; `details` includes the script, requested family, substitution, and failure reason.
- A 404 usually means `assetBaseUrl` points at the application root while viewer assets were copied into a subdirectory.
- A CSP error requires the self-host origin in `font-src`; custom URL sources also need CORS permission.
- Arabic joining and Indic shaping require the original GSUB/GPOS tables. The packaged subsets preserve all layout features.
- Search/copy order comes from logical text maps, not glyph placement. RTL/LTR visual bidi therefore does not reverse copied text.
