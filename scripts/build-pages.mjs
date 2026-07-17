import { execFileSync } from "node:child_process";
import { copyFile, cp, mkdir, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const docs = resolve(root, "docs");
const output = resolve(docs, ".vitepress/dist");
const demoOutput = resolve(root, "examples/react/dist");
const base = pagesBase();
const demoBase = `${base}demo/`;
const env = { ...process.env, PAGES_BASE: base, ZRIMO_BASE_PATH: demoBase };

run("node", ["scripts/build-wasm.mjs"]);
run("npm", ["run", "build", "--workspace", "@zrimo/viewer"]);
run("npm", ["run", "build", "--workspace", "@zrimo/example-react"]);
run("node", ["node_modules/vitepress/bin/vitepress.js", "build", "docs"]);

await mkdir(output, { recursive: true });
await rm(resolve(output, "demo"), { recursive: true, force: true });
await cp(demoOutput, resolve(output, "demo"), { recursive: true });
// VitePress preview resolves `/demo/` through its clean-URL fallback before it
// considers a copied directory index. The sibling file keeps local preview and
// static GitHub Pages routing equivalent while the React assets stay in /demo/.
await copyFile(resolve(demoOutput, "index.html"), resolve(output, "demo.html"));
await writeFile(resolve(output, ".nojekyll"), "");

console.log(`GitHub Pages bundle built at ${output} with base ${base}`);

function run(command, args) {
  execFileSync(command, args, { cwd: root, env, stdio: "inherit" });
}

function pagesBase() {
  if (process.env.PAGES_BASE) return normalizeBase(process.env.PAGES_BASE);
  const [owner, repository] = (process.env.GITHUB_REPOSITORY ?? "").split("/");
  if (owner && repository) {
    return repository.toLowerCase() === `${owner}.github.io`.toLowerCase()
      ? "/"
      : `/${repository}/`;
  }
  return "/";
}

function normalizeBase(value) {
  const trimmed = value.trim();
  return `/${trimmed.replace(/^\/+|\/+$/g, "")}/`.replace("//", "/");
}
