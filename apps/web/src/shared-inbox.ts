const SHARED_CACHE = "hexscope-shared";
const SHARED_TTL = 24 * 60 * 60 * 1000;
const SHARED_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

/** Remove expired and malformed inbox records before reading a submission. */
async function purgeSharedCache(cache: Cache, now = Date.now()): Promise<void> {
  const keys = await cache.keys();
  const scope = new URL("./", location.href);
  const legacyUrl = new URL("shared-file", scope).href;
  const legacyConsumedUrl = new URL("shared-file-consumed", scope).href;
  if (keys.some((key) => key.url === legacyConsumedUrl)) {
    const legacyFiles = keys.filter((key) => key.url === legacyUrl);
    const removedFiles = await Promise.allSettled(legacyFiles.map((key) => cache.delete(key)));
    if (removedFiles.every((result) => result.status === "fulfilled")) {
      await Promise.allSettled(keys.filter((key) => key.url === legacyConsumedUrl).map((key) => cache.delete(key)));
    }
  } else {
    const legacy = keys.find((key) => key.url === legacyUrl);
    if (legacy) {
      const response = await cache.match(legacy);
      if (!response) await cache.delete(legacy);
      else {
        const recordedAt = response.headers.get("x-received-at");
        const at = recordedAt === null ? Number.NaN : Number(recordedAt);
        const valid = Number.isSafeInteger(at) && at >= 0 && at <= now;
        const receivedAt = valid ? at : now;
        if (!valid) {
          const headers = new Headers(response.headers);
          headers.set("x-received-at", String(now));
          await cache.put(legacy, new Response(response.body, { headers }));
        }
        if (now - receivedAt >= SHARED_TTL) await cache.delete(legacy);
      }
    }
  }

  const grouped = new Map<string, Array<{ key: Request; index: number | null; consumed: boolean }>>();
  const malformed: Request[] = [];
  for (const key of keys) {
    if (key.url === legacyUrl || key.url === legacyConsumedUrl) continue;
    const url = new URL(key.url);
    const path = url.pathname.startsWith(scope.pathname) ? url.pathname.slice(scope.pathname.length) : "";
    const match = /^shared\/([0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12})\/(\d+|_consumed)$/.exec(path);
    if (url.origin !== scope.origin || !match) {
      malformed.push(key);
      continue;
    }
    const group = grouped.get(match[1]) ?? [];
    group.push({ key, index: match[2] === "_consumed" ? null : Number(match[2]), consumed: match[2] === "_consumed" });
    grouped.set(match[1], group);
  }
  await Promise.allSettled(malformed.map((key) => cache.delete(key)));
  for (const [id, entries] of grouped) {
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
        index === null ||
        recordedIndex === null ||
        Number(recordedIndex) !== index ||
        !Number.isSafeInteger(at) ||
        at < 0 ||
        at > now ||
        now - at >= SHARED_TTL
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

function sharedFileName(response: Response, fallback: string): string {
  const encoded = response.headers.get("x-file-name");
  if (!encoded) return fallback;
  try {
    return decodeURIComponent(encoded);
  } catch {
    return fallback;
  }
}

/** A marker makes consume-once effective even if deleting several cache keys is interrupted. */
async function consumeSharedFiles(cache: Cache, keys: Array<Request | URL>, marker: URL): Promise<void> {
  await cache.put(marker, new Response("", { headers: { "x-submission-id": marker.pathname.split("/").at(-2) ?? "legacy" } }));
  try {
    for (const key of keys) await cache.delete(key);
  } catch {
    // Keep the marker so the remaining bytes cannot be opened twice; the next
    // worker or inbox visit removes them.
    throw new Error("shared submission could not be consumed");
  }
  try {
    await cache.delete(marker);
  } catch {
    // The marker contains no user data and expires with its submission.
  }
}

type SharedInboxResult = { files: File[] } | { reason: "missing" | "failed" };

/** Read and consume one complete share-target submission. */
export async function receiveSharedFiles(id: string | null): Promise<SharedInboxResult> {
  if (!("caches" in window)) return { reason: "failed" };
  try {
    const cache = await caches.open(SHARED_CACHE);
    await purgeSharedCache(cache);
    if (id === "1") {
      const scope = new URL("./", location.href);
      const key = new URL("shared-file", scope);
      const marker = new URL("shared-file-consumed", scope);
      if (await cache.match(marker)) return { reason: "missing" };
      const response = await cache.match(key);
      if (!response) return { reason: "missing" };
      const receivedAt = response.headers.get("x-received-at");
      const at = receivedAt === null ? Number.NaN : Number(receivedAt);
      const now = Date.now();
      if (!Number.isSafeInteger(at) || at < 0 || at > now || now - at >= SHARED_TTL) return { reason: "missing" };
      const name = sharedFileName(response, "shared file");
      const type = response.headers.get("x-file-type") ?? response.headers.get("Content-Type") ?? "";
      const file = new File([await response.blob()], name, { type });
      await consumeSharedFiles(cache, [key], marker);
      return { files: [file] };
    }
    if (!id || !SHARED_ID.test(id)) return { reason: "missing" };

    const scope = new URL("./", location.href);
    const prefix = new URL(`shared/${id}/`, scope);
    const marker = new URL(`shared/${id}/_consumed`, scope);
    const consumed = await cache.match(marker);
    const keys = (await cache.keys())
      .filter((key) => key.url.startsWith(prefix.href) && /^\d+$/.test(new URL(key.url).pathname.slice(prefix.pathname.length)))
      .sort((a, b) => Number(new URL(a.url).pathname.slice(prefix.pathname.length)) - Number(new URL(b.url).pathname.slice(prefix.pathname.length)));
    const entries = await Promise.all(keys.map(async (key) => ({ key, response: await cache.match(key) })));
    const count = Number(entries[0]?.response?.headers.get("x-file-count"));
    const now = Date.now();
    const complete = !consumed && Number.isInteger(count) && count > 0 && count <= 50 && entries.length === count && entries.every(({ key, response }, index) => {
      const fileIndex = Number(new URL(key.url).pathname.slice(prefix.pathname.length));
      const receivedAt = response?.headers.get("x-received-at");
      const at = receivedAt === null || receivedAt === undefined ? Number.NaN : Number(receivedAt);
      return (
        response !== undefined &&
        fileIndex === index &&
        response.headers.get("x-submission-id") === id &&
        response.headers.get("x-file-index") === String(index) &&
        response.headers.get("x-file-count") === String(count) &&
        Number.isSafeInteger(at) &&
        at >= 0 &&
        at <= now &&
        now - at < SHARED_TTL
      );
    });
    if (!complete) {
      await Promise.allSettled(keys.map((key) => cache.delete(key)));
      return { reason: "missing" };
    }
    const files: File[] = [];
    for (let index = 0; index < entries.length; index++) {
      const response = entries[index].response!;
      const blob = await response.blob();
      const type = response.headers.get("x-file-type") ?? "";
      files.push(new File([blob], sharedFileName(response, `shared-file-${index + 1}`), { type }));
    }
    await consumeSharedFiles(cache, keys, marker);
    return { files };
  } catch {
    return { reason: "failed" };
  }
}
