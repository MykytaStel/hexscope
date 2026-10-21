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
const SHARE_TTL = 24 * 60 * 60 * 1000;
const SHARE_MAX_FILES = 50;
const SHARE_MAX_BYTES = 400 * 1024 * 1024;
const SHARE_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

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
      await purgeSharedInbox(await caches.open(SHARED));
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

// Each share sheet submission gets its own bounded inbox. Files stay local;
// the redirect names a complete group only after every cache write succeeds.
async function receiveShare(req) {
  let cache;
  try {
    cache = await caches.open(SHARED);
    await purgeSharedInbox(cache);
  } catch {
    return shareFailure("storage");
  }
  let files;
  try {
    const form = await req.formData();
    files = form.getAll("file").filter((file) => typeof file !== "string" && file && typeof file.size === "number" && typeof file.slice === "function");
  } catch {
    return shareFailure("receive");
  }
  if (files.length === 0) return shareFailure("empty");
  if (files.length > SHARE_MAX_FILES) return shareFailure("count");
  const total = files.reduce((sum, file) => sum + file.size, 0);
  if (!Number.isFinite(total) || files.some((file) => !Number.isFinite(file.size) || file.size < 0) || total > SHARE_MAX_BYTES)
    return shareFailure("size");

  let id;
  try {
    const existing = await cache.keys();
    for (let attempt = 0; attempt < 5; attempt++) {
      const candidate = crypto.randomUUID().toLowerCase();
      if (SHARE_ID.test(candidate) && !existing.some((key) => key.url.startsWith(new URL(`shared/${candidate}/`, self.registration.scope).href))) {
        id = candidate;
        break;
      }
    }
  } catch {
    return shareFailure("storage");
  }
  if (!id) return shareFailure("storage");

  const receivedAt = String(Date.now());
  const written = [];
  try {
    for (let index = 0; index < files.length; index++) {
      const file = files[index];
      const key = new URL(`shared/${id}/${index}`, self.registration.scope);
      written.push(key);
      await cache.put(
        key,
        new Response(file, {
          headers: {
            "x-submission-id": id,
            "x-file-index": String(index),
            "x-file-count": String(files.length),
            "x-file-name": encodeURIComponent(file.name || `shared-file-${index + 1}`),
            "x-file-type": file.type || "",
            "x-received-at": receivedAt,
          },
        }),
      );
    }
  } catch {
    await Promise.allSettled(written.map((key) => cache.delete(key)));
    return shareFailure("storage");
  }
  return Response.redirect(new URL(`./?shared=${id}`, self.registration.scope).href, 303);
}

function shareFailure(reason) {
  return Response.redirect(new URL(`./?share-error=${reason}`, self.registration.scope).href, 303);
}

/** Drop expired or malformed entries. A legacy entry receives its timestamp once. */
async function purgeSharedInbox(cache, now = Date.now()) {
  const keys = await cache.keys();
  const legacyUrl = new URL("shared-file", self.registration.scope).href;
  const legacyConsumedUrl = new URL("shared-file-consumed", self.registration.scope).href;
  const legacy = keys.find((key) => key.url === legacyUrl);
  if (keys.some((key) => key.url === legacyConsumedUrl)) {
    const legacyFiles = keys.filter((key) => key.url === legacyUrl);
    const removedFiles = await Promise.allSettled(legacyFiles.map((key) => cache.delete(key)));
    if (removedFiles.every((result) => result.status === "fulfilled")) {
      await Promise.allSettled(keys.filter((key) => key.url === legacyConsumedUrl).map((key) => cache.delete(key)));
    }
  } else if (legacy) {
    const response = await cache.match(legacy);
    if (response) {
      const recordedAt = response.headers.get("x-received-at");
      const at = recordedAt === null ? Number.NaN : Number(recordedAt);
      const valid = Number.isSafeInteger(at) && at >= 0 && at <= now;
      const receivedAt = valid ? at : now;
      if (!valid) {
        const headers = new Headers(response.headers);
        headers.set("x-received-at", String(now));
        await cache.put(legacy, new Response(response.body, { headers }));
      }
      if (now - receivedAt >= SHARE_TTL) await cache.delete(legacy);
    } else await cache.delete(legacy);
  }

  const groups = new Map();
  const malformed = [];
  const basePath = new URL(self.registration.scope).pathname;
  for (const key of keys) {
    if (key.url === legacyUrl || key.url === legacyConsumedUrl) continue;
    const url = new URL(key.url);
    const relative = url.pathname.startsWith(basePath) ? url.pathname.slice(basePath.length) : "";
    const match = /^shared\/([0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12})\/(\d+|_consumed)$/.exec(relative);
    if (url.origin !== new URL(self.registration.scope).origin || !match) {
      malformed.push(key);
      continue;
    }
    const group = groups.get(match[1]) ?? [];
    group.push({ key, index: match[2] === "_consumed" ? null : Number(match[2]), consumed: match[2] === "_consumed" });
    groups.set(match[1], group);
  }
  await Promise.allSettled(malformed.map((key) => cache.delete(key)));
  for (const [id, entries] of groups) {
    let invalid = entries.some(({ consumed }) => consumed);
    for (const { key, index, consumed } of entries) {
      if (consumed) continue;
      const response = await cache.match(key);
      const recordedAt = response?.headers.get("x-received-at");
      const recordedIndex = response?.headers.get("x-file-index");
      const at = recordedAt === null || recordedAt === undefined ? Number.NaN : Number(recordedAt);
      if (
        !response ||
        response.headers.get("x-submission-id") !== id ||
        index === null || recordedIndex === null || Number(recordedIndex) !== index ||
        !Number.isSafeInteger(at) ||
        at < 0 ||
        at > now ||
        now - at >= SHARE_TTL
      ) {
        invalid = true;
        break;
      }
    }
    if (invalid) {
      const files = entries.filter(({ consumed }) => !consumed);
      const markers = entries.filter(({ consumed }) => consumed);
      const removedFiles = await Promise.allSettled(files.map(({ key }) => cache.delete(key)));
      if (markers.length === 0 || removedFiles.every((result) => result.status === "fulfilled")) {
        await Promise.allSettled(markers.map(({ key }) => cache.delete(key)));
      }
    }
  }
}
