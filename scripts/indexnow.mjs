// Tells search engines that take IndexNow (Bing, Yandex, Seznam, Naver…)
// that the site's pages changed, right after a deploy, so they need not wait
// to crawl it. The key is public by design: the file in public/ that holds
// it proves the site asked.
//
//   node scripts/indexnow.mjs apps/web/dist
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const dist = process.argv[2] ?? "apps/web/dist";
const keyFile = readdirSync(dist).find((f) => /^[0-9a-f]{32}\.txt$/.test(f));
if (!keyFile) throw new Error(`no IndexNow key file in ${dist}`);
const key = readFileSync(join(dist, keyFile), "utf8").trim();
const urls = [...readFileSync(join(dist, "sitemap.xml"), "utf8").matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1]);
const { origin, host } = new URL(urls[0]);

const res = await fetch("https://api.indexnow.org/indexnow", {
  method: "POST",
  headers: { "Content-Type": "application/json; charset=utf-8" },
  body: JSON.stringify({ host, key, keyLocation: `${origin}/${keyFile}`, urlList: urls }),
});
// 200 and 202 both mean it was taken; anything else is worth a look, not a failed deploy.
console.log(`IndexNow: ${res.status} for ${urls.length} pages of ${host}`);
if (!res.ok) console.log(`::warning::IndexNow answered ${res.status}: ${await res.text()}`);
