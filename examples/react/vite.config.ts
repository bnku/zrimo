import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  // Serve/copy the built package assets at the same root used by assetBaseUrl.
  publicDir: "../../packages/viewer/dist",
});
