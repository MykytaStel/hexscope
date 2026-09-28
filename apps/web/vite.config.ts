import { createHash } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";

const page = (name: string) => fileURLToPath(new URL(name, import.meta.url));

/** Files the service worker does not keep: the samples have their own list,
 * and the rest is for crawlers, link previews and the host. */
const NOT_KEPT = /^(samples\/|sw\.js$|_headers$|robots\.txt$|sitemap\.xml$|og\.png$)/;

/**
 * Writes into the built sw.js every file of this build, and a version made
 * from their contents: the whole app is kept at the first visit — both
 * WebAssembly builds, the player and the guides, not only what that visit
 * opened — and a new deploy replaces the old files instead of piling up.
 */
function precache(): Plugin {
  let out = "";
  return {
    name: "hexscope-precache",
    apply: "build",
    configResolved(config) {
      out = config.build.outDir.startsWith("/") ? config.build.outDir : join(config.root, config.build.outDir);
    },
    closeBundle() {
      const walk = (dir: string): string[] =>
        readdirSync(dir, { withFileTypes: true }).flatMap((d) => (d.isDirectory() ? walk(join(dir, d.name)) : [join(dir, d.name)]));
      const files = walk(out)
        .map((f) => relative(out, f).split("\\").join("/"))
        .filter((f) => !NOT_KEPT.test(f))
        .sort();
      const hash = createHash("sha256");
      for (const f of files) hash.update(f).update(readFileSync(join(out, f)));
      // Pages by the address the host serves them at: it redirects `.html`.
      const urls = files.map((f) => (f === "index.html" ? "./" : f.replace(/\.html$/, "")));
      const sw = join(out, "sw.js");
      const text = readFileSync(sw, "utf8");
      const built = text
        .replace('const BUILD = "dev";', `const BUILD = "${hash.digest("hex").slice(0, 12)}";`)
        .replace("const FILES = [];", `const FILES = ${JSON.stringify(urls)};`);
      if (built === text) throw new Error("sw.js: the build's placeholders are missing");
      writeFileSync(sw, built);
    },
  };
}

/**
 * The headers the host sends for every page (public/_headers, the "/*"
 * block), sent by `vite preview` too: the end-to-end tests then run under
 * the same Content-Security-Policy as the site, and catch what it blocks.
 */
function siteHeaders(): Record<string, string> {
  const lines = readFileSync(page("public/_headers"), "utf8").split("\n");
  const out: Record<string, string> = {};
  let inAll = false;
  for (const line of lines) {
    if (!line.startsWith(" ")) inAll = line.trim() === "/*";
    else if (inAll) {
      const at = line.indexOf(":");
      if (at > 0) out[line.slice(0, at).trim()] = line.slice(at + 1).trim();
    }
  }
  return out;
}

export default defineConfig({
  preview: { headers: siteHeaders() },
  // Relative asset paths so the build works from any subpath, e.g. GitHub Pages.
  base: "./",
  worker: { format: "es" },
  plugins: [precache()],
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
        blackout: page("black-out-a-pdf.html"),
        email: page("is-this-email-real.html"),
        screenshot: page("what-a-screenshot-gives-away.html"),
      },
    },
  },
});
