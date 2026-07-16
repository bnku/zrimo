import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const artifacts = resolve(root, "artifacts");
await mkdir(artifacts, { recursive: true });
const packed = JSON.parse(
  execFileSync(
    "npm",
    [
      "pack",
      "--workspace",
      "@docs-viewer-wasm/viewer",
      "--json",
      "--pack-destination",
      artifacts,
    ],
    { cwd: root, encoding: "utf8" },
  ),
)[0];
const names = new Set(packed.files.map((file) => file.path));
const required = [
  "dist/index.js",
  "dist/index.d.ts",
  "dist/headless.js",
  "dist/headless.d.ts",
  "dist/worker.js",
  "dist/worker.d.ts",
  "dist/styles.css",
  "dist/workers/pdf-worker.js",
  "dist/workers/image-worker.js",
  "dist/workers/legacy-converter-worker.js",
  "dist/assets/pdf/index_bg.wasm",
  "dist/assets/image/index_bg.wasm",
  "dist/assets/legacy/index_bg.wasm",
  "dist/fonts/manifest.json",
  "LICENSE-MIT",
  "LICENSE-APACHE",
  "THIRD_PARTY_NOTICES.md",
];
const missing = required.filter((file) => !names.has(file));
const forbidden = packed.files
  .map((file) => file.path)
  .filter(
    (file) =>
      /(?:^|\/)(?:src|test|corpus|\.env)(?:\/|$)/.test(file) ||
      file.endsWith(".map") ||
      /(?:secret|credential|private[-_]?key)/i.test(file),
  );
if (missing.length || forbidden.length)
  throw new Error(
    `Packed content invalid; missing=${missing.join(",")}; forbidden=${forbidden.join(",")}`,
  );
const tarball = resolve(artifacts, packed.filename);
const bytes = await readFile(tarball);
const sha256 = createHash("sha256").update(bytes).digest("hex");
await writeFile(
  resolve(artifacts, "SHA256SUMS"),
  `${sha256}  ${basename(tarball)}\n`,
);
const report = {
  schemaVersion: 1,
  name: packed.name,
  version: packed.version,
  filename: packed.filename,
  size: packed.size,
  unpackedSize: packed.unpackedSize,
  shasum: packed.shasum,
  integrity: packed.integrity,
  sha256,
  fileCount: packed.entryCount,
  requiredExportsPresent: true,
  forbiddenFiles: [],
  consumers: [],
};

const consumer = resolve(root, ".cache/pack-consumer");
await rm(consumer, { recursive: true, force: true });
await mkdir(consumer, { recursive: true });
await writeFile(
  resolve(consumer, "package.json"),
  `${JSON.stringify({ private: true, type: "module" }, null, 2)}\n`,
);
execFileSync("npm", ["install", "--ignore-scripts", "--no-audit", tarball], {
  cwd: consumer,
  stdio: "inherit",
});
await writeFile(
  resolve(consumer, "consumer.mjs"),
  [
    'import { ViewerClient } from "@docs-viewer-wasm/viewer";',
    'import { ViewerError } from "@docs-viewer-wasm/viewer/headless";',
    'import { WorkerRpcClient } from "@docs-viewer-wasm/viewer/worker";',
    "const client = ViewerClient.create();",
    "await client.destroy();",
    "if (!ViewerError || !WorkerRpcClient) throw new Error('missing export');",
  ].join("\n"),
);
execFileSync("node", ["consumer.mjs"], { cwd: consumer, stdio: "inherit" });
report.consumers.push("plain-esm-ssr");

await writeFile(
  resolve(consumer, "browser.ts"),
  [
    'import { ViewerClient } from "@docs-viewer-wasm/viewer";',
    'import "@docs-viewer-wasm/viewer/styles.css";',
    "const client = ViewerClient.create({ assetBaseUrl: new URL('/', location.href) });",
    "globalThis.viewerClient = client;",
  ].join("\n"),
);
await writeFile(
  resolve(consumer, "consumer.ts"),
  [
    'import { ViewerClient, type ViewerApi } from "@docs-viewer-wasm/viewer";',
    'import type { WorkerRequestOptions } from "@docs-viewer-wasm/viewer/worker";',
    "const client = ViewerClient.create();",
    "const viewer: ViewerApi = client.createViewer();",
    "const request: WorkerRequestOptions = { timeoutMs: 1000 };",
    "void viewer; void request;",
  ].join("\n"),
);
await writeFile(
  resolve(consumer, "tsconfig.json"),
  `${JSON.stringify(
    {
      compilerOptions: {
        target: "ES2022",
        module: "NodeNext",
        moduleResolution: "NodeNext",
        lib: ["ES2022", "DOM", "WebWorker"],
        strict: true,
        skipLibCheck: true,
        noEmit: true,
      },
      include: ["consumer.ts"],
    },
    null,
    2,
  )}\n`,
);
execFileSync(
  "node",
  [resolve(root, "node_modules/typescript/bin/tsc"), "-p", "tsconfig.json"],
  { cwd: consumer, stdio: "inherit" },
);
report.consumers.push("typescript-strict");

execFileSync(
  resolve(root, "node_modules/esbuild/bin/esbuild"),
  [
    "browser.ts",
    "--bundle",
    "--format=esm",
    "--platform=browser",
    "--outdir=dist-esbuild",
  ],
  { cwd: consumer, stdio: "inherit" },
);
report.consumers.push("angular-esbuild");

await writeFile(
  resolve(consumer, "index.html"),
  '<!doctype html><div id="viewer"></div><script type="module" src="/browser.ts"></script>\n',
);
execFileSync(
  "node",
  [
    resolve(root, "node_modules/vite/bin/vite.js"),
    "build",
    "--outDir",
    "dist-vite",
  ],
  { cwd: consumer, stdio: "inherit" },
);
report.consumers.push("vite");

await writeFile(
  resolve(consumer, "webpack-entry.js"),
  'import { ViewerClient } from "@docs-viewer-wasm/viewer"; export default ViewerClient;\n',
);
await writeFile(
  resolve(consumer, "webpack.config.cjs"),
  "module.exports = { mode: 'production', entry: './webpack-entry.js', output: { filename: 'bundle.js', path: require('node:path').resolve(__dirname, 'dist-webpack') }, experiments: { asyncWebAssembly: true } };\n",
);
execFileSync(
  "node",
  [
    resolve(root, "node_modules/webpack-cli/bin/cli.js"),
    "--config",
    "webpack.config.cjs",
  ],
  { cwd: consumer, stdio: "inherit" },
);
report.consumers.push("webpack");

const nextApp = resolve(consumer, "next-app");
await mkdir(resolve(nextApp, "app"), { recursive: true });
await writeFile(
  resolve(nextApp, "package.json"),
  `${JSON.stringify({ private: true, scripts: { build: "next build" } }, null, 2)}\n`,
);
await writeFile(
  resolve(nextApp, "app/layout.js"),
  "export default function Layout({ children }) { return <html><body>{children}</body></html>; }\n",
);
await writeFile(
  resolve(nextApp, "app/page.js"),
  'import { ViewerClient } from "@docs-viewer-wasm/viewer"; export default function Page() { return <main data-viewer={typeof ViewerClient}>SSR-safe viewer</main>; }\n',
);
execFileSync(
  "node",
  [resolve(root, "node_modules/next/dist/bin/next"), "build", "--webpack"],
  {
    cwd: nextApp,
    stdio: "inherit",
    env: { ...process.env, NEXT_TELEMETRY_DISABLED: "1" },
  },
);
report.consumers.push("nextjs-webpack");

await writeFile(
  resolve(artifacts, "pack-report.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
console.log(
  `Packed ${packed.name}@${packed.version}: ${packed.entryCount} files, ${packed.size} bytes; ${report.consumers.length} consumer gates passed.`,
);
