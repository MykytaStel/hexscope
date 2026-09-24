// hexscope's service worker. After the first visit the app keeps working
// offline, and the installed app can receive a file from the share sheet.
// It fetches nothing the page does not ask for, and sends nothing anywhere.
const CACHE = "hexscope-v1";
const SHARED = "hexscope-shared";

self.addEventListener("install", () => self.skipWaiting());

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      for (const key of await caches.keys()) {
        if (key !== CACHE && key !== SHARED) await caches.delete(key);
      }
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;
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
  const hit = await cache.match(req);
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
    const hit = (await cache.match(req, { ignoreSearch: true })) ?? (req.mode === "navigate" ? await cache.match(new URL("./", self.registration.scope)) : undefined);
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
