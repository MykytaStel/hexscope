// What search engines read, written at build time into dist/: a Ukrainian
// copy of every page at /uk/…, links between the two languages, a
// description of each page as structured data, the sitemap and robots.txt.
// The site's address is set once, in SITE: buying a domain changes one line.
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { GUIDE_UK } from "./src/guide-i18n";
import { registerLazyTranslations, translateText } from "./src/i18n";
import "./src/i18n-uk";

/** Where the site lives; HEXSCOPE_SITE overrides it for a build. */
export const SITE = (process.env.HEXSCOPE_SITE ?? "https://hexscope.org").replace(/\/$/, "");

const DECODED: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " };
const decode = (s: string) =>
  s.replace(/&(#x[0-9a-f]+|#\d+|\w+);/gi, (m, e: string) =>
    e[0] === "#" ? String.fromCodePoint(e[1].toLowerCase() === "x" ? Number.parseInt(e.slice(2), 16) : Number(e.slice(1))) : (DECODED[e] ?? m),
  );
const encode = (s: string, quote = false) => {
  const out = s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  return quote ? out.replace(/"/g, "&quot;") : out;
};
const uk = (text: string) => translateText(text, "uk");

/** The page's address: `/` for the app, `/name` for the rest, as the host serves them. */
export const pathOf = (file: string) => (file === "index.html" ? "/" : `/${file.replace(/\.html$/, "")}`);
const ukPath = (path: string) => (path === "/" ? "/uk/" : `/uk${path}`);

/** The page the host answers with, and a 404, for an address that is no page. */
const NOT_FOUND = "404.html";

/** Links to pages by the address the host serves them at, not `name.html`, which it redirects. */
const cleanLinks = (html: string) => html.replace(/\bhref="([a-z0-9-]+)\.html(#[^"]*)?"/g, 'href="$1$2"');

/**
 * The page not found, at any depth: the host serves it for /a/b/c too, so its
 * files and links start at the root (or at /uk/ for the Ukrainian one).
 */
const fromRoot = (html: string, base: "/" | "/uk/") =>
  html
    .replace(/\b(src|href)="(?:\.\.?\/)+(?=[^"])/g, '$1="/')
    .replace(/\bhref="\.\/"/g, `href="${base}"`)
    .replace(/\bhref="([a-z0-9-]+)"/g, `href="${base}$1"`);

/** The same page in Ukrainian: its words, its language, its own address, its files one folder up. */
function inUkrainian(html: string, path: string): string {
  const parts = html.split(/(<script\b[\s\S]*?<\/script>|<style\b[\s\S]*?<\/style>)/i);
  return parts
    .map((part, i) => {
      if (i % 2 === 1) return part;
      return part
        .replace(/>([^<]+)</g, (m, text: string) => (text.trim() ? `>${encode(uk(decode(text)))}<` : m))
        .replace(/\b(content|alt|title|aria-label|placeholder)="([^"]*)"/g, (m, name: string, value: string) =>
          name === "content" && !/^[A-Za-z]/.test(value) ? m : `${name}="${encode(uk(decode(value)), true)}"`,
        );
    })
    .join("")
    .replace(/<html lang="en"/, '<html lang="uk"')
    .replace(new RegExp(`(rel="canonical" href=")${SITE}[^"]*"`), `$1${SITE}${ukPath(path)}"`)
    .replace(new RegExp(`(property="og:url" content=")${SITE}[^"]*"`), `$1${SITE}${ukPath(path)}"`)
    // Files of the site are one folder up; pages stay beside it, in Ukrainian.
    .replace(/\b(src|href)="\.\/(?!["?#])/g, '$1="../')
    // «Name» around a button's name in italics: no space inside the quotes.
    .replace(/«\s+(<(?:em|strong|a)\b)/g, "«$1")
    .replace(/(<\/(?:em|strong|a)>)\s+»/g, "$1»");
}

function between(html: string, open: RegExp, close: string): string {
  const m = open.exec(html);
  if (!m) return "";
  const start = m.index + m[0].length;
  return decode(html.slice(start, html.indexOf(close, start)).replace(/<[^>]+>/g, "")).trim();
}

/** What the app does, in the words people search with. */
const FEATURES = [
  "EXIF and metadata viewer for photos",
  "Remove the location and camera data from a photo",
  "Remove the location and metadata from a phone's videos",
  "See whether a picture says AI made it, and its prompt",
  "Remove metadata from PDF files",
  "Remove metadata from Word, Excel and PowerPoint files",
  "Remove metadata from LibreOffice and RTF documents",
  "Find text hidden under black boxes in a PDF",
  "Check an email's headers for signs of phishing",
  "Tell a file's real type when its name says another",
];

/** When a page was first and last changed: the dates of its first and latest commit. */
interface Dates {
  published: string;
  modified: string;
}

/** The dates of `file` in `root`'s history; a build outside a clone, or of a page not committed yet, says today. */
function committed(root: string, file: string, today: string): Dates {
  try {
    const days = execFileSync("git", ["log", "--follow", "--format=%cs", "--", file], { cwd: root, encoding: "utf8" })
      .split("\n")
      .filter(Boolean);
    if (days.length > 0) return { published: days[days.length - 1], modified: days[0] };
  } catch {
    // No git, or not a clone: the build's own day.
  }
  return { published: today, modified: today };
}

/** Who makes the site, and where else it is. */
const publisher = {
  "@type": "Organization",
  name: "hexscope",
  url: `${SITE}/`,
  logo: `${SITE}/icon-512.png`,
  sameAs: ["https://github.com/MykytaStel/hexscope", "https://crates.io/crates/hexscope-cli"],
};

/**
 * What the page is, for search engines: the app as a free web application,
 * every other page an article with its dates and its place under the home page.
 */
function structured(html: string, url: string, lang: "en" | "uk", version: string, dates: Dates): string {
  const description = decode(/name="description" content="([^"]*)"/.exec(html)?.[1] ?? "");
  const title = between(html, /<title>/, "</title>").replace(/ — hexscope$/, "");
  const home = `${SITE}${lang === "uk" ? "/uk/" : "/"}`;
  const app = url === home;
  const headline = between(html, /<h1[^>]*>/, "</h1>") || title;
  const data = app
    ? {
        "@context": "https://schema.org",
        "@type": "WebApplication",
        name: "hexscope",
        url,
        description,
        applicationCategory: "SecurityApplication",
        operatingSystem: "Any",
        browserRequirements: "A browser with WebAssembly",
        isAccessibleForFree: true,
        offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
        softwareVersion: version,
        inLanguage: lang,
        screenshot: `${SITE}/analyzer-preview.png`,
        license: "https://github.com/MykytaStel/hexscope/blob/main/LICENSE",
        publisher,
        featureList: FEATURES.map((f) => (lang === "uk" ? uk(f) : f)),
      }
    : {
        "@context": "https://schema.org",
        "@graph": [
          {
            "@type": "Article",
            headline,
            description,
            url,
            inLanguage: lang,
            datePublished: dates.published,
            dateModified: dates.modified,
            image: `${SITE}/og.jpg`,
            isPartOf: { "@type": "WebSite", name: "hexscope", url: home },
            author: publisher,
            publisher,
          },
          {
            "@type": "BreadcrumbList",
            itemListElement: [
              { "@type": "ListItem", position: 1, name: "hexscope", item: home },
              { "@type": "ListItem", position: 2, name: headline, item: url },
            ],
          },
        ],
      };
  const json = JSON.stringify(data).replace(/</g, "\\u003c");
  return `<script type="application/ld+json">${json}</script>`;
}

const alternates = (path: string) =>
  [
    `<link rel="alternate" hreflang="en" href="${SITE}${path}" />`,
    `<link rel="alternate" hreflang="uk" href="${SITE}${ukPath(path)}" />`,
    `<link rel="alternate" hreflang="x-default" href="${SITE}${path}" />`,
  ].join("\n    ");

/** The language of a link preview, and the other one it exists in. */
const locales = (lang: "en" | "uk") =>
  [
    `<meta property="og:locale" content="${lang === "uk" ? "uk_UA" : "en_US"}" />`,
    `<meta property="og:locale:alternate" content="${lang === "uk" ? "en_US" : "uk_UA"}" />`,
  ].join("\n    ");

/**
 * Writes the Ukrainian pages, the language links, structured data, sitemap
 * and robots.txt into `out`; each page is dated by its source's history in `root`.
 */
export function writeSeo(out: string, pages: readonly string[], version: string, root: string): void {
  registerLazyTranslations(GUIDE_UK);
  mkdirSync(join(out, "uk"), { recursive: true });
  const today = new Date().toISOString().slice(0, 10);
  const urls: string[] = [];
  for (const file of pages) {
    const path = pathOf(file);
    const html = cleanLinks(readFileSync(join(out, file), "utf8"));
    const ukHtml = inUkrainian(html, path);
    if (file === NOT_FOUND) {
      // Not a page to find: no language links, no description, not in the sitemap.
      writeFileSync(join(out, file), fromRoot(html, "/"));
      writeFileSync(join(out, "uk", file), fromRoot(ukHtml, "/uk/"));
      continue;
    }
    // A Ukrainian page is its English one, in other words: the same dates.
    const dates = committed(root, file, today);
    // An article has a page of its own in each language, so theme.js opens the
    // reader's one instead of translating this one (the app translates itself).
    const article = (h: string) => (file === "index.html" ? h : h.replace(/<html lang="(en|uk)"/, '<html lang="$1" data-page="article"'));
    const head = (h: string, url: string, lang: "en" | "uk") =>
      article(h).replace("</head>", `    ${alternates(path)}\n    ${locales(lang)}\n    ${structured(h, url, lang, version, dates)}\n  </head>`);
    writeFileSync(join(out, file), head(html, `${SITE}${path}`, "en"));
    writeFileSync(join(out, "uk", file), head(ukHtml, `${SITE}${ukPath(path)}`, "uk"));
    const links = ["en", "uk", "x-default"]
      .map((lang) => `<xhtml:link rel="alternate" hreflang="${lang}" href="${SITE}${lang === "uk" ? ukPath(path) : path}"/>`)
      .join("");
    for (const loc of [`${SITE}${path}`, `${SITE}${ukPath(path)}`]) {
      urls.push(`  <url><loc>${loc}</loc><lastmod>${dates.modified}</lastmod>${links}</url>`);
    }
  }
  writeFileSync(
    join(out, "sitemap.xml"),
    `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${urls.join("\n")}\n</urlset>\n`,
  );
  writeFileSync(join(out, "robots.txt"), `User-agent: *\nAllow: /\n\nSitemap: ${SITE}/sitemap.xml\n`);
}
