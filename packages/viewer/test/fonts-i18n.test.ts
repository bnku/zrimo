import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type {
  FontEnvironment,
  FontRequest,
  TextRun,
  ViewerWarning,
} from "../src/index.js";
import {
  FontManager,
  fontRequestsForRuns,
  resolveTranslations,
  scriptForCodepoint,
} from "../src/index.js";

describe("font discovery", () => {
  it("classifies every promised script and groups mixed runs", () => {
    const examples = new Map([
      ["A", "latin"],
      ["Я", "cyrillic"],
      ["漢", "cjk"],
      ["あ", "japanese"],
      ["한", "korean"],
      ["ش", "arabic"],
      ["न", "devanagari"],
      ["ব", "bengali"],
      ["ગ", "gujarati"],
      ["ਗ", "gurmukhi"],
      ["ଓ", "odia"],
      ["த", "tamil"],
      ["త", "telugu"],
      ["ಕ", "kannada"],
      ["മ", "malayalam"],
    ] as const);
    for (const [character, script] of examples)
      assert.equal(scriptForCodepoint(character.codePointAt(0)!), script);
    const requests = fontRequestsForRuns([
      run([...examples.keys()].join(""), "Requested Sans"),
    ]);
    assert.deepEqual(
      new Set(requests.map((request) => request.script)),
      new Set(examples.values()),
    );
    assert.equal(
      requests.every((request) => request.family === "Requested Sans"),
      true,
    );
  });
});

describe("font policy", () => {
  it("loads only encountered packaged scripts and reuses cached faces", async () => {
    const harness = fakeFonts(false);
    const requests: string[] = [];
    const manager = new FontManager({
      fetch: async (input) => {
        requests.push(String(input));
        return new Response(new Uint8Array([1, 2, 3]));
      },
      assetBaseUrl: new URL("https://assets.example/viewer/"),
      environment: harness.environment,
    });
    const warnings: ViewerWarning[] = [];
    const mixed = [run("العربية हिन्दी")];
    await manager.ensureRuns(mixed, (warning) => warnings.push(warning));
    await manager.ensureRuns(mixed, (warning) => warnings.push(warning));
    assert.equal(requests.length, 2);
    assert.equal(
      requests.some((url) => url.endsWith("noto-sans-arabic.woff2")),
      true,
    );
    assert.equal(
      requests.some((url) => url.endsWith("noto-sans-devanagari.woff2")),
      true,
    );
    assert.equal(warnings.length, 0);
    assert.equal(harness.faces.added.length, 2);
    await manager.destroy();
    assert.equal(harness.faces.deleted.length, 2);
  });

  it("offline mode performs no fetch and emits one stable warning", async () => {
    const harness = fakeFonts(false);
    let fetches = 0;
    const warnings: ViewerWarning[] = [];
    const manager = new FontManager({
      policy: { mode: "offline" },
      fetch: async () => {
        fetches += 1;
        throw new Error("unexpected fetch");
      },
      environment: harness.environment,
    });
    await manager.ensureRuns([run("漢字")], (warning) =>
      warnings.push(warning),
    );
    await manager.ensureRuns([run("漢字")], (warning) =>
      warnings.push(warning),
    );
    assert.equal(fetches, 0);
    assert.equal(warnings.length, 1);
    assert.equal(warnings[0]?.code, "font-unavailable");
  });

  it("uses registered bytes before custom/system/pack resolution", async () => {
    const harness = fakeFonts(false);
    let fetches = 0;
    const manager = new FontManager({
      policy: { mode: "custom", resolver: () => null },
      registered: [
        {
          family: "Host Arabic",
          scripts: ["arabic"],
          source: new Uint8Array([9, 9, 9]),
        },
      ],
      fetch: async () => {
        fetches += 1;
        throw new Error("unexpected fetch");
      },
      environment: harness.environment,
    });
    await manager.ensureRuns([run("سلام", "Host Arabic")], () => {});
    assert.equal(fetches, 0);
    assert.equal(harness.faces.added.length, 1);
  });

  it("uses an available system font without fetching a fallback", async () => {
    const harness = fakeFonts(true);
    let fetches = 0;
    const manager = new FontManager({
      fetch: async () => {
        fetches += 1;
        throw new Error("unexpected fetch");
      },
      environment: harness.environment,
    });
    await manager.ensureRuns([run("Привет", "Host UI")], () => {});
    assert.equal(fetches, 0);
    assert.equal(harness.faces.added.length, 0);
  });

  it("turns a missing custom font URL into a non-fatal warning", async () => {
    const harness = fakeFonts(false);
    const warnings: ViewerWarning[] = [];
    const manager = new FontManager({
      policy: {
        mode: "custom",
        resolver: () => ({
          family: "Missing",
          source: "https://fonts.example/missing.woff2",
        }),
      },
      fetch: async () => new Response(null, { status: 404 }),
      environment: harness.environment,
    });
    await manager.ensureRuns([run("বাংলা")], (warning) =>
      warnings.push(warning),
    );
    assert.equal(
      warnings.some((warning) => warning.code === "font-unavailable"),
      true,
    );
    assert.equal(harness.faces.added.length, 0);
  });

  it("reports a corrupted custom font without throwing document rendering", async () => {
    const harness = fakeFonts(false, true);
    const warnings: ViewerWarning[] = [];
    let seen: FontRequest | undefined;
    const manager = new FontManager({
      policy: {
        mode: "custom",
        resolver: (request) => {
          seen = request;
          return { family: "Broken", source: new Uint8Array([0]) };
        },
      },
      fetch: async () => new Response(),
      environment: harness.environment,
    });
    await manager.ensureRuns([run("தமிழ்")], (warning) =>
      warnings.push(warning),
    );
    assert.equal(seen?.script, "tamil");
    assert.deepEqual(
      warnings.map((warning) => warning.code),
      ["font-unavailable", "font-unavailable"],
    );
  });
});

describe("locale dictionaries", () => {
  it("falls back through English and applies host overrides", () => {
    assert.equal(
      resolveTranslations("en", undefined).download,
      "Download original",
    );
    assert.equal(
      resolveTranslations("ru", undefined).download,
      "Скачать оригинал",
    );
    const custom = resolveTranslations("ru", {
      download: "Сохранить исходник",
    });
    assert.equal(custom.download, "Сохранить исходник");
    assert.equal(custom.search, "Поиск");
    assert.equal(Object.isFrozen(custom), true);
  });
});

function run(text: string, fontFamily?: string): TextRun {
  return {
    text,
    x: 0,
    y: 0,
    width: 100,
    height: 20,
    ...(fontFamily ? { fontFamily } : {}),
  };
}

function fakeFonts(
  systemAvailable: boolean,
  rejectLoads = false,
): {
  environment: FontEnvironment;
  faces: FakeFontSet;
} {
  const faces = new FakeFontSet(systemAvailable);
  class FakeFontFace {
    constructor(
      readonly family: string,
      readonly source: ArrayBuffer,
    ) {}

    async load(): Promise<FakeFontFace> {
      if (rejectLoads) throw new Error("corrupted font");
      return this;
    }
  }
  return {
    environment: {
      FontFace: FakeFontFace as unknown as typeof FontFace,
      fonts: faces as unknown as FontFaceSet,
    },
    faces,
  };
}

class FakeFontSet {
  readonly added: FontFace[] = [];
  readonly deleted: FontFace[] = [];

  constructor(readonly systemAvailable: boolean) {}

  check(): boolean {
    return this.systemAvailable;
  }

  add(face: FontFace): this {
    this.added.push(face);
    return this;
  }

  delete(face: FontFace): boolean {
    this.deleted.push(face);
    return true;
  }
}
