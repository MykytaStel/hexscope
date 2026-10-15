// hexscope's service worker. After the first visit the app keeps working
// offline, and the installed app can receive a file from the share sheet.
// It fetches only this site's own files, and leaves the visit counter's
// requests (/api/) to the browser's own cache, which is what makes them rare.

// The build writes in its version and its files (vite.config.ts): a new
// deploy is a new cache. The one before it stays until the next deploy, for
// a page still open on it that has yet to load a part — the clean copy's
// check, a WebAssembly build — which the server no longer has.
const BUILD = "dev";
const FILES = [];
const CACHE = `hexscope-${BUILD}`;
const SHARED = "hexscope-shared";
const ORDER = "hexscope-order";

// The samples are small, and the landing page offers them: kept from the
// start, so they open offline too — unless the connection asks to save data.
const SAMPLES = [
  "ai-picture.png", "broken.png", "budget.xlsx", "cropped.jpg", "deflate-demo.zip", "hello.wasm", "holiday.jpg", "lease.odt",
  "message.eml", "phishing.eml", "phishing.msg", "photo.heic", "photo.jpg", "plan.doc", "progressive.jpg", "redacted.pdf",
  "report.docx", "report.pdf", "sample.png", "video.mov", "wifi.png",
].map((n) => new URL(`samples/${n}`, self.registration.scope).href);

const APP = FILES.map((n) => new URL(n, self.registration.scope).href);

// The whole app, so it all works offline after one visit; the samples too,
// unless the connection asks to save data. One file that fails to come does
// not keep the rest out: it is fetched when first asked for.
self.addEventListener("install", (event) => {
  self.skipWaiting();
  const wanted = self.navigator?.connection?.saveData ? [] : [...APP, ...SAMPLES];
  event.waitUntil(
    caches.open(CACHE).then((cache) =>
      Promise.allSettled(
        wanted.map(async (url) => {
          const res = await fetch(url, { cache: "no-cache" });
          if (res.ok && !res.redirected) await cache.put(url, res);
        }),
      ),
    ),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      const builds = keys.filter((key) => key !== CACHE && key !== SHARED && key !== ORDER);
      // The build this one follows; before the order was kept, whichever is there.
      const book = await caches.open(ORDER);
      const where = new URL("order", self.registration.scope).href;
      const known = await book
        .match(where)
        .then((res) => (res ? res.json() : []))
        .catch(() => []);
      const before = known.filter((key) => key !== CACHE && builds.includes(key)).at(-1) ?? builds.at(-1);
      for (const key of builds) if (key !== before) await caches.delete(key);
      await book.put(where, new Response(JSON.stringify(before ? [before, CACHE] : [CACHE])));
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin || url.pathname.startsWith("/api/")) return;
  if (req.method === "POST" && url.pathname.endsWith("/share")) {
    event.respondWith(receiveShare(req));
    return;
  }
  if (req.method !== "GET") return;
  // Hashed assets never change: cache first. Everything else network first,
  // so a new deploy shows at once and the cache is only the offline fallback.
  event.respondWith(url.pathname.includes("/assets/") ? cacheFirst(req) : networkFirst(req));
});

async function cacheFirst(req) {
  const cache = await caches.open(CACHE);
  // Kept by the install, asked for by the page as a module with an Origin
  // header: a server's Vary would make the two differ, for the same bytes.
  // A page open on the build before asks for its own files: they are there.
  const hit = (await cache.match(req, { ignoreVary: true })) ?? (await caches.match(req, { ignoreVary: true }));
  if (hit) return hit;
  const res = await fetch(req);
  if (res.ok) await cache.put(req, res.clone());
  return res;
}

async function networkFirst(req) {
  const cache = await caches.open(CACHE);
  try {
    const res = await fetch(req);
    if (res.ok) await cache.put(req, res.clone());
    return res;
  } catch (err) {
    // A page asked for as `name.html` is kept as `name`, the address the
    // host serves it at; anything else unknown opens the app.
    const clean = new URL(req.url);
    clean.pathname = clean.pathname.replace(/(index)?\.html$/, "");
    const hit =
      (await cache.match(req, { ignoreSearch: true, ignoreVary: true })) ??
      (req.mode === "navigate" ? ((await cache.match(clean.href, { ignoreSearch: true, ignoreVary: true })) ?? (await cache.match(new URL("./", self.registration.scope)))) : undefined);
    if (hit) return hit;
    throw err;
  }
}

// The share sheet posts the file here. It waits in a cache until the page,
// opened at ?shared, takes it: a file never touches the network.
async function receiveShare(req) {
  const form = await req.formData();
  const file = form.get("file");
  if (file && typeof file !== "string") {
    const cache = await caches.open(SHARED);
    await cache.put(
      new URL("shared-file", self.registration.scope),
      new Response(file, { headers: { "x-file-name": encodeURIComponent(file.name) } }),
    );
  }
  return Response.redirect(new URL("./?shared=1", self.registration.scope).href, 303);
}
