// Tells search engines that take IndexNow (Bing, Yandex, Seznam, Naver…)
// which of the site's pages changed, right after a deploy, so they need not
// wait to crawl it. The key is public by design: the file in public/ that
// holds it proves the site asked.
//
//   node scripts/indexnow.mjs apps/web/dist [sitemap-before.xml]
//
// Given the sitemap the site had before the deploy, only the pages that are
// new or have a newer date are named; without it, all of them.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const dist = process.argv[2] ?? "apps/web/dist";
const before = process.argv[3];
const keyFile = readdirSync(dist).find((f) => /^[0-9a-f]{32}\.txt$/.test(f));
if (!keyFile) throw new Error(`no IndexNow key file in ${dist}`);
const key = readFileSync(join(dist, keyFile), "utf8").trim();

/** Each page of a sitemap, with its date. */
const pages = (xml) => new Map([...xml.matchAll(/<loc>([^<]+)<\/loc><lastmod>([^<]+)<\/lastmod>/g)].map((m) => [m[1], m[2]]));
const now = pages(readFileSync(join(dist, "sitemap.xml"), "utf8"));
const was = before && existsSync(before) ? pages(readFileSync(before, "utf8")) : new Map();
const urls = was.size === 0 ? [...now.keys()] : [...now].filter(([loc, day]) => was.get(loc) !== day).map(([loc]) => loc);
if (urls.length === 0) {
  console.log("IndexNow: no page changed");
  process.exit(0);
}
const { origin, host } = new URL(urls[0]);

const res = await fetch("https://api.indexnow.org/indexnow", {
  method: "POST",
  headers: { "Content-Type": "application/json; charset=utf-8" },
  body: JSON.stringify({ host, key, keyLocation: `${origin}/${keyFile}`, urlList: urls }),
});
// 200 and 202 both mean it was taken; anything else is worth a look, not a failed deploy.
console.log(`IndexNow: ${res.status} for ${urls.length} of ${now.size} pages of ${host}`);
if (!res.ok) console.log(`::warning::IndexNow answered ${res.status}: ${await res.text()}`);
