import { defineConfig } from "vite";

export default defineConfig({
  // Relative asset paths so the build works from any subpath, e.g. GitHub Pages.
  base: "./",
  worker: { format: "es" },
  build: { target: "es2022" },
});
