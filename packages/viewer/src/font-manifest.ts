import type { FontScript } from "./contracts.js";

export interface FallbackFontPack {
  readonly id: string;
  readonly scripts: readonly FontScript[];
  readonly file: string;
  readonly unicodeRange: string;
  readonly family: string;
  readonly license: "OFL-1.1";
  readonly source: string;
}

const coreSource =
  "https://github.com/notofonts/noto-fonts/tree/ffebf8c1ee449e544955a7e813c54f9b73848eac";
const cjkSource =
  "https://github.com/notofonts/noto-cjk/tree/f8d157532fbfaeda587e826d4cd5b21a49186f7c";

export const fallbackFontPacks: readonly FallbackFontPack[] = Object.freeze([
  pack(
    "latin-cyrillic",
    ["latin", "cyrillic"],
    "noto-sans-latin-cyrillic.woff2",
    "U+0000-024F,U+0300-036F,U+0400-052F,U+1E00-1EFF",
  ),
  pack(
    "arabic",
    ["arabic"],
    "noto-sans-arabic.woff2",
    "U+0600-06FF,U+0750-077F,U+08A0-08FF,U+FB50-FDFF,U+FE70-FEFF",
  ),
  pack(
    "devanagari",
    ["devanagari"],
    "noto-sans-devanagari.woff2",
    "U+0900-097F,U+A8E0-A8FF",
  ),
  pack("bengali", ["bengali"], "noto-sans-bengali.woff2", "U+0980-09FF"),
  pack("gurmukhi", ["gurmukhi"], "noto-sans-gurmukhi.woff2", "U+0A00-0A7F"),
  pack("gujarati", ["gujarati"], "noto-sans-gujarati.woff2", "U+0A80-0AFF"),
  pack("odia", ["odia"], "noto-sans-odia.woff2", "U+0B00-0B7F"),
  pack("tamil", ["tamil"], "noto-sans-tamil.woff2", "U+0B80-0BFF"),
  pack("telugu", ["telugu"], "noto-sans-telugu.woff2", "U+0C00-0C7F"),
  pack("kannada", ["kannada"], "noto-sans-kannada.woff2", "U+0C80-0CFF"),
  pack("malayalam", ["malayalam"], "noto-sans-malayalam.woff2", "U+0D00-0D7F"),
  pack(
    "cjk",
    ["cjk"],
    "noto-sans-cjk.woff2",
    "U+2E80-2FFF,U+3000-303F,U+31C0-31EF,U+3400-4DBF,U+4E00-9FFF,U+F900-FAFF",
  ),
  pack(
    "japanese",
    ["japanese"],
    "noto-sans-japanese.woff2",
    "U+3040-30FF,U+31F0-31FF",
  ),
  pack(
    "korean",
    ["korean"],
    "noto-sans-korean.woff2",
    "U+1100-11FF,U+3130-318F,U+AC00-D7AF",
  ),
]);

function pack(
  id: string,
  scripts: readonly FontScript[],
  file: string,
  unicodeRange: string,
): FallbackFontPack {
  return Object.freeze({
    id,
    scripts: Object.freeze([...scripts]),
    file,
    unicodeRange,
    family: "Zrimo Noto",
    license: "OFL-1.1",
    source:
      id === "cjk" || id === "japanese" || id === "korean"
        ? cjkSource
        : coreSource,
  });
}
