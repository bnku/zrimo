import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  base: process.env.ZRIMO_BASE_PATH ?? "/",
  // Serve/copy the built package assets at the same root used by assetBaseUrl.
  publicDir: "../../packages/viewer/dist",
});
