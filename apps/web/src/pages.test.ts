// Every public page but the app repeats the same head and top bar, copied
// from one to the next: a canonical address left from the page it was copied
// from, or a page the build never lists, would ship without a word. This
// reads each page and holds it to what search engines and the build need.
import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { GUIDES } from "../guides";

const root = new URL("../", import.meta.url);
const vite = readFileSync(new URL("vite.config.ts", root), "utf8");
const pages = readdirSync(root).filter((f) => f.endsWith(".html") && f !== "index.html" && f !== "404.html");
const meta = (html: string, key: string) =>
  new RegExp(`<meta (?:name|property)="${key}" content="([^"]*)"`).exec(html)?.[1];

describe.each(pages)("%s", (file) => {
  const html = readFileSync(new URL(file, root), "utf8");
  const address = `https://hexscope.org/${file.replace(/\.html$/, "")}`;

  it("names its own address, not the page it was copied from", () => {
    expect(/<link rel="canonical" href="([^"]+)"/.exec(html)?.[1]).toBe(address);
    expect(meta(html, "og:url")).toBe(address);
  });

  it("says what it is to search engines and link previews", () => {
    expect(/<title>[^<]*hexscope<\/title>/.test(html)).toBe(true);
    expect(meta(html, "description")?.length ?? 0).toBeGreaterThan(60);
    for (const key of ["og:type", "og:title", "og:description", "og:image", "twitter:card"]) expect(meta(html, key), key).toBeTruthy();
  });

  it("sets its theme first, and runs a script of the site's own", () => {
    expect(html.indexOf('<script src="./theme.js"></script>')).toBeLessThan(html.indexOf("<title>"));
    // The DEFLATE story runs its player; every other page the pages' script and stylesheet.
    if (file === "deflate.html") expect(html).toMatch(/<script type="module" src="\/src\/[\w-]+\.ts"><\/script>/);
    else {
      expect(html).toContain('<script type="module" src="/src/page.ts"></script>');
      expect(html).toContain('<link rel="stylesheet" href="/src/style.css" />');
    }
  });

  it("is built, and a guide is in the list of guides", () => {
    expect(vite).toContain(`page("${file}")`);
    if (GUIDES.some((g) => g.file === file)) expect(html).toContain("<ul data-guides></ul>");
  });
});
