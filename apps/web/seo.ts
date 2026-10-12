// What search engines read, written at build time into dist/: a Ukrainian
// copy of every page at /uk/…, links between the two languages, a
// description of each page as structured data, the sitemap and robots.txt.
// The site's address is set once, in SITE: buying a domain changes one line.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { GUIDE_UK } from "./src/guide-i18n";
import { registerLazyTranslations, translateText } from "./src/i18n";

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
    .replace(/\b(src|href)="\.\/(?!["?#])/g, '$1="../');
}

function between(html: string, open: RegExp, close: string): string {
  const m = open.exec(html);
  if (!m) return "";
  const start = m.index + m[0].length;
  return decode(html.slice(start, html.indexOf(close, start)).replace(/<[^>]+>/g, "")).trim();
}

/** What the page is, for search engines: the app as a free web application, every other page an article. */
function structured(html: string, url: string, lang: "en" | "uk", version: string): string {
  const description = decode(/name="description" content="([^"]*)"/.exec(html)?.[1] ?? "");
  const title = between(html, /<title>/, "</title>").replace(/ — hexscope$/, "");
  const site = { "@type": "WebSite", name: "hexscope", url: `${SITE}/` };
  const app = url === `${SITE}/` || url === `${SITE}/uk/`;
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
      }
    : null;
  const article = {
    "@context": "https://schema.org",
    "@type": "Article",
    headline: between(html, /<h1[^>]*>/, "</h1>") || title,
    description,
    url,
    inLanguage: lang,
    isPartOf: site,
    publisher: { "@type": "Organization", name: "hexscope", url: `${SITE}/`, logo: `${SITE}/icon-512.png` },
  };
  const json = JSON.stringify(data ?? article).replace(/</g, "\\u003c");
  return `<script type="application/ld+json">${json}</script>`;
}

const alternates = (path: string) =>
  [
    `<link rel="alternate" hreflang="en" href="${SITE}${path}" />`,
    `<link rel="alternate" hreflang="uk" href="${SITE}${ukPath(path)}" />`,
    `<link rel="alternate" hreflang="x-default" href="${SITE}${path}" />`,
  ].join("\n    ");

/** Writes the Ukrainian pages, the language links, structured data, sitemap and robots.txt into `out`. */
export function writeSeo(out: string, pages: readonly string[], version: string): void {
  registerLazyTranslations(GUIDE_UK);
  mkdirSync(join(out, "uk"), { recursive: true });
  const today = new Date().toISOString().slice(0, 10);
  const urls: string[] = [];
  for (const file of pages) {
    const path = pathOf(file);
    const html = readFileSync(join(out, file), "utf8");
    const ukHtml = inUkrainian(html, path);
    const head = (h: string, url: string, lang: "en" | "uk") =>
      h.replace("</head>", `    ${alternates(path)}\n    ${structured(h, url, lang, version)}\n  </head>`);
    writeFileSync(join(out, file), head(html, `${SITE}${path}`, "en"));
    writeFileSync(join(out, "uk", file), head(ukHtml, `${SITE}${ukPath(path)}`, "uk"));
    const links = ["en", "uk", "x-default"]
      .map((lang) => `<xhtml:link rel="alternate" hreflang="${lang}" href="${SITE}${lang === "uk" ? ukPath(path) : path}"/>`)
      .join("");
    for (const loc of [`${SITE}${path}`, `${SITE}${ukPath(path)}`]) {
      urls.push(`  <url><loc>${loc}</loc><lastmod>${today}</lastmod>${links}</url>`);
    }
  }
  writeFileSync(
    join(out, "sitemap.xml"),
    `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${urls.join("\n")}\n</urlset>\n`,
  );
  writeFileSync(join(out, "robots.txt"), `User-agent: *\nAllow: /\n\nSitemap: ${SITE}/sitemap.xml\n`);
}
