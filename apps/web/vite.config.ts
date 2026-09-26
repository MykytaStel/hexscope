import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const page = (name: string) => fileURLToPath(new URL(name, import.meta.url));

export default defineConfig({
  // Relative asset paths so the build works from any subpath, e.g. GitHub Pages.
  base: "./",
  worker: { format: "es" },
  // The commit a report names, so a bug can be matched to the code it met.
  define: { __BUILD__: JSON.stringify((process.env.GITHUB_SHA ?? "dev").slice(0, 7)) },
  build: {
    target: "es2022",
    rollupOptions: {
      // The app, the story page that explains DEFLATE with its player, and
      // the guides, which are plain pages.
      input: {
        main: page("index.html"),
        deflate: page("deflate.html"),
        location: page("remove-location-from-photo.html"),
        pdf: page("pdf-hidden-versions.html"),
        png: page("png-wont-open.html"),
        documents: page("check-document-before-sending.html"),
        hidden: page("hidden-text-in-pdf.html"),
      },
    },
  },
});
