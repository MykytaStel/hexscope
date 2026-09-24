import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const page = (name: string) => fileURLToPath(new URL(name, import.meta.url));

export default defineConfig({
  // Relative asset paths so the build works from any subpath, e.g. GitHub Pages.
  base: "./",
  worker: { format: "es" },
  build: {
    target: "es2022",
    rollupOptions: {
      // The app, and the story page that explains DEFLATE with its player.
      input: { main: page("index.html"), deflate: page("deflate.html") },
    },
  },
});
