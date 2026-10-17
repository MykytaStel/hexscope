// What a visitor's browser gets from the live site, checked against what
// the site promises: no script, style, picture or frame from anywhere else,
// and the headers that keep it so. The tests run on a local build, so they
// cannot see what the host adds on its way out — Cloudflare, for one, puts
// its analytics script into pages for browsers when its Web Analytics is on.
//
//   node scripts/live-check.mjs [https://hexscope.org]
//
// Exits 1 on anything that breaks the promise; what is only worth knowing
// (a reporting header the host adds) is a warning.

const site = new URL(process.argv[2] ?? "https://hexscope.org");
const PAGES = ["/", "/uk/", "/remove-metadata-from-photo", "/uk/remove-metadata-from-photo", "/your-files-stay-private", "/support"];
// As a browser asks: hosts add things only for browsers.
const BROWSER = {
  "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36",
  Accept: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
};
const github = process.env.GITHUB_ACTIONS === "true";

let failed = 0;
function fail(page, message) {
  failed += 1;
  console.log(github ? `::error title=Live site ${page}::${message}` : `✗ ${page}: ${message}`);
}
function warn(page, message) {
  console.log(github ? `::warning title=Live site ${page}::${message}` : `! ${page}: ${message}`);
}

/** Every address a tag in the page makes the browser load or run. */
function loads(html) {
  const out = [];
  for (const [, tag, attrs] of html.matchAll(/<(script|link|img|iframe|source|video|audio|embed|object)\b([^>]*)>/gi)) {
    const attr = (name) => attrs.match(new RegExp(`\\s${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, "i"))?.slice(1).find((v) => v !== undefined);
    const name = tag.toLowerCase();
    if (name === "link") {
      // A link to another page, or to the page in another language, loads nothing.
      const rel = (attr("rel") ?? "").toLowerCase();
      if (!/stylesheet|preload|modulepreload|prefetch|icon|manifest/.test(rel)) continue;
    }
    const url = attr("src") ?? attr("href") ?? attr("data");
    if (url !== undefined) out.push({ tag: name, url });
    else if (name === "script" && !/application\/ld\+json/i.test(attr("type") ?? "")) out.push({ tag: name, url: "(inline)" });
  }
  return out;
}

for (const path of PAGES) {
  const url = new URL(path, site);
  let response;
  try {
    response = await fetch(url, { headers: BROWSER, redirect: "follow" });
  } catch (error) {
    fail(path, `could not be fetched: ${error.message}`);
    continue;
  }
  if (response.status !== 200) {
    fail(path, `answered ${response.status}`);
    continue;
  }
  const csp = response.headers.get("content-security-policy") ?? "";
  for (const rule of ["default-src 'self'", "connect-src 'self'", "object-src 'none'"]) {
    if (!csp.includes(rule)) fail(path, `its Content-Security-Policy lacks ${rule}`);
  }
  if (!/script-src 'self'( 'wasm-unsafe-eval')?;/.test(csp)) fail(path, "its Content-Security-Policy lets scripts come from elsewhere");
  if (response.headers.get("referrer-policy") !== "no-referrer") fail(path, "has no Referrer-Policy: no-referrer");
  if (!response.headers.get("strict-transport-security")) fail(path, "has no Strict-Transport-Security");
  if (response.headers.get("nel") || response.headers.get("report-to")) {
    warn(path, "the host adds NEL / Report-To: a browser may report failed visits to the host's own collector");
  }
  for (const { tag, url: address } of loads(await response.text())) {
    if (address === "(inline)") {
      fail(path, `has an inline <${tag}>, which the policy blocks`);
      continue;
    }
    const from = new URL(address, url);
    if (from.origin !== site.origin && from.protocol !== "data:" && from.protocol !== "blob:") {
      fail(path, `loads <${tag}> from ${from.origin}${from.pathname}`);
    }
  }
}

console.log(failed ? `${failed} problem${failed === 1 ? "" : "s"} on ${site.origin}` : `${PAGES.length} pages of ${site.origin} load nothing from elsewhere`);
process.exit(failed ? 1 : 0);
