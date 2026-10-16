// Every public page has a Ukrainian copy, made at build from the English one
// fragment by fragment (seo.ts): a fragment without a translation stays in
// English there. These tests name each one, so a new guide cannot ship half
// translated.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { GUIDES, guideList } from "../guides";
import { GUIDE_UK } from "./guide-i18n";
import { registerLazyTranslations, translateText } from "./i18n";
import "./i18n-uk";

registerLazyTranslations(GUIDE_UK);

const root = new URL("../", import.meta.url);
const ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " };
const decode = (s: string) => s.replace(/&(\w+);/g, (m, e: string) => ENTITIES[e] ?? m);

/**
 * The words of a page a reader sees, as the build translates them: the text
 * between tags, the title and the descriptions. Button and menu names in
 * <em> and anything in <code> may stay as the program shows them.
 */
function fragments(html: string): string[] {
  const body = html
    .replace(/<script\b[\s\S]*?<\/script>|<style\b[\s\S]*?<\/style>/gi, "")
    .replace(/<(em|code)\b[^>]*>[\s\S]*?<\/\1>/g, "<x/>");
  const texts = [...body.matchAll(/>([^<]+)</g)].map((m) => m[1]);
  const described = [...html.matchAll(/(?:name="description"|property="og:(?:title|description)") content="([^"]*)"/g)].map((m) => m[1]);
  return [...texts, ...described].map((t) => decode(t).replace(/\s+/g, " ").trim()).filter(Boolean);
}

/** A fragment in words, not a name, a number or an address: one lowercase word of three letters or more. */
const inWords = (t: string) => !/[/@]|\.\w{2,}\b/.test(t) && /\b[a-z]{3,}\b/.test(t.replace(/\bhexscope\b/g, ""));

const pages = readdirSync(root).filter((f) => f.endsWith(".html") && f !== "index.html");

describe("the Ukrainian copy of every public page", () => {
  it.each(pages)("%s has a translation for each of its fragments", (file) => {
    const left = fragments(readFileSync(new URL(file, root), "utf8")).filter((t) => inWords(t) && translateText(t, "uk") === t);
    expect(left).toEqual([]);
  });

  it("names every guide in Ukrainian in the lists of guides", () => {
    expect(GUIDES.map((g) => g.title).filter((t) => translateText(t, "uk") === t)).toEqual([]);
  });
});

describe("the list of the other guides", () => {
  it("leaves out the page it is on, and starts with the guides on its topic", () => {
    const list = guideList("remove-metadata-from-video.html");
    expect(list).not.toContain('href="remove-metadata-from-video.html"');
    expect(list.indexOf("what-a-screenshot-gives-away.html")).toBeLessThan(list.indexOf("remove-metadata-from-pdf.html"));
    expect(list.match(/<li>/g)).toHaveLength(GUIDES.length - 1);
  });

  it("lists every guide on a page that is not one", () => {
    expect(guideList("404.html").match(/<li>/g)).toHaveLength(GUIDES.length);
  });

  it("is in every guide, and every guide is a page", () => {
    for (const g of GUIDES) expect(readFileSync(new URL(g.file, root), "utf8")).toContain("<ul data-guides></ul>");
  });
});

// The Ukrainian pages as the build writes them, fragment joined to fragment:
// a quote opened in one and closed in the next, a space lost at a join.
const built = new URL("../dist/uk/", import.meta.url);

describe.skipIf(!existsSync(built))("the punctuation of every built Ukrainian page", () => {
  const read = (file: string) =>
    decode(
      readFileSync(new URL(file, built), "utf8")
        .replace(/<script\b[\s\S]*?<\/script>|<style\b[\s\S]*?<\/style>|<code\b[\s\S]*?<\/code>/gi, " ")
        .replace(/<\/?(?:p|li|ul|ol|h[1-6]|div|section|nav|header|footer|main|article|aside|figure|figcaption|dt|dd|td|th|tr|br)\b[^>]*>/gi, " ")
        .replace(/<[^>]+>/g, ""),
    ).replace(/\s+/g, " ");
  const files = existsSync(built) ? readdirSync(built).filter((f) => f.endsWith(".html")) : [];

  it.each(files)("%s closes every quote it opens", (file) => {
    const text = read(file);
    expect([(text.match(/«/g) ?? []).length, (text.match(/»/g) ?? []).length]).toEqual([
      (text.match(/«/g) ?? []).length,
      (text.match(/«/g) ?? []).length,
    ]);
  });

  it.each(files)("%s keeps a space after each sentence and around each dash", (file) => {
    const text = read(file);
    const glued = [...text.matchAll(/[a-zа-яіїєґ’][.!?:;][А-ЯІЇЄҐ]|[A-Za-zА-Яа-яІіЇїЄєҐґ]—|—[A-Za-zА-Яа-яІіЇїЄєҐґ]/g)].map((m) =>
      text.slice(Math.max(0, m.index - 20), m.index + 20),
    );
    expect(glued).toEqual([]);
  });
});
