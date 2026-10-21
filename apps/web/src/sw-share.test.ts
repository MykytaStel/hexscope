import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { describe, expect, it } from "vitest";

const workerSource = readFileSync(new URL("../public/sw.js", import.meta.url), "utf8");
const scope = "https://hexscope.test/app/";
const ttl = 24 * 60 * 60 * 1000;
const ids = ["00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000002"];

type CacheRequest = string | URL | Request;

function keyOf(request: CacheRequest): string {
  return new URL(typeof request === "string" ? request : request instanceof URL ? request.href : request.url, scope).href;
}

class MemoryCache {
  readonly values = new Map<string, Response>();
  writes = 0;
  quotaAt: number | null = null;

  async match(request: CacheRequest, options?: CacheQueryOptions): Promise<Response | undefined> {
    const key = keyOf(request);
    const exact = this.values.get(key);
    if (exact) return exact.clone();
    if (!options?.ignoreSearch) return undefined;
    const wanted = new URL(key);
    for (const [candidate, response] of this.values) {
      const parsed = new URL(candidate);
      if (parsed.origin === wanted.origin && parsed.pathname === wanted.pathname) return response.clone();
    }
    return undefined;
  }

  async put(request: CacheRequest, response: Response): Promise<void> {
    this.writes++;
    const key = keyOf(request);
    this.values.set(key, response.clone());
    if (this.quotaAt === this.writes) throw new DOMException("Storage quota exceeded", "QuotaExceededError");
  }

  async delete(request: CacheRequest): Promise<boolean> {
    return this.values.delete(keyOf(request));
  }

  async keys(): Promise<Request[]> {
    return [...this.values.keys()].map((key) => new Request(key));
  }
}

class MemoryCacheStorage {
  readonly stores = new Map<string, MemoryCache>();

  async open(name: string): Promise<MemoryCache> {
    let cache = this.stores.get(name);
    if (!cache) {
      cache = new MemoryCache();
      this.stores.set(name, cache);
    }
    return cache;
  }

  async keys(): Promise<string[]> {
    return [...this.stores.keys()];
  }

  async delete(name: string): Promise<boolean> {
    return this.stores.delete(name);
  }

  async match(request: CacheRequest, options?: CacheQueryOptions): Promise<Response | undefined> {
    for (const cache of this.stores.values()) {
      const found = await cache.match(request, options);
      if (found) return found;
    }
    return undefined;
  }
}

function makeFile(name: string, type: string, contents: string): Blob & { name: string } {
  const file = new Blob([contents], { type }) as Blob & { name: string };
  Object.defineProperty(file, "name", { value: name });
  return file;
}

function harness() {
  let now = 1_798_000_000_000;
  const cacheStorage = new MemoryCacheStorage();
  const listeners = new Map<string, (event: any) => void>();
  const generatedIds = [...ids];
  let skipped = 0;
  let fetched = 0;
  const self = {
    location: new URL("https://hexscope.test/app/sw.js"),
    registration: { scope },
    navigator: { connection: { saveData: true } },
    clients: { claim: async () => undefined },
    skipWaiting: async () => { skipped++; },
    addEventListener: (type: string, listener: (event: any) => void) => listeners.set(type, listener),
  };
  class ClockDate extends Date {
    static now(): number {
      return now;
    }
  }
  runInNewContext(workerSource, {
    self,
    caches: cacheStorage,
    Response,
    Headers,
    Request,
    URL,
    Date: ClockDate,
    crypto: { randomUUID: () => generatedIds.shift() ?? ids[0] },
    fetch: async () => { fetched++; return new Response("offline"); },
  });

  async function dispatch(type: string, detail: Record<string, unknown> = {}): Promise<unknown> {
    const waits: Promise<unknown>[] = [];
    let response: Promise<unknown> | undefined;
    const event = {
      ...detail,
      waitUntil: (promise: Promise<unknown>) => waits.push(Promise.resolve(promise)),
      respondWith: (promise: Promise<unknown>) => (response = Promise.resolve(promise)),
    };
    listeners.get(type)?.(event);
    await Promise.all(waits);
    return response ? await response : undefined;
  }

  async function share(files: Array<Blob & { name: string; size: number; type: string }>): Promise<Response> {
    const request = {
      method: "POST",
      url: `${scope}share`,
      formData: async () => ({
        get: (name: string) => (name === "file" ? files[0] ?? null : null),
        getAll: (name: string) => (name === "file" ? files : []),
      }),
    };
    return (await dispatch("fetch", { request })) as Response;
  }

  return { cacheStorage, dispatch, share, now: () => now, advance: (by: number) => (now += by), skipped: () => skipped, fetched: () => fetched };
}

describe("the PWA shared-file inbox", () => {
  it("installs without fetching share data when the browser asks to save data", async () => {
    const app = harness();
    await app.dispatch("install");
    expect(app.skipped()).toBe(1);
    expect(app.fetched()).toBe(0);
  });

  it("stores every file in order with duplicate names and MIME types intact", async () => {
    const app = harness();
    const response = await app.share([makeFile("same.txt", "text/plain", "first"), makeFile("same.txt", "text/markdown", "second")]);
    const id = ids[0];
    expect(response.headers.get("location")).toBe(`${scope}?shared=${id}`);
    const inbox = await app.cacheStorage.open("hexscope-shared");
    const records = await inbox.keys();
    expect(records.map((key) => key.url)).toEqual([
      `${scope}shared/${id}/0`,
      `${scope}shared/${id}/1`,
    ]);
    const first = await inbox.match(records[0]);
    const second = await inbox.match(records[1]);
    expect(first?.headers.get("x-file-name")).toBe(encodeURIComponent("same.txt"));
    expect(first?.headers.get("x-file-type")).toBe("text/plain");
    expect(first?.headers.get("x-file-index")).toBe("0");
    expect(first?.headers.get("x-received-at")).toBe(String(app.now()));
    expect(await first?.text()).toBe("first");
    expect(second?.headers.get("x-file-type")).toBe("text/markdown");
    expect(await second?.text()).toBe("second");
  });

  it("keeps separate submissions isolated and rejects an empty submission", async () => {
    const app = harness();
    const first = await app.share([makeFile("a.txt", "text/plain", "a")]);
    const second = await app.share([makeFile("b.txt", "text/plain", "b")]);
    expect(first.headers.get("location")).toBe(`${scope}?shared=${ids[0]}`);
    expect(second.headers.get("location")).toBe(`${scope}?shared=${ids[1]}`);
    const empty = await app.share([]);
    expect(empty.headers.get("location")).toBe(`${scope}?share-error=empty`);
    const inbox = await app.cacheStorage.open("hexscope-shared");
    expect((await inbox.keys()).map((r) => r.url)).toEqual([
      `${scope}shared/${ids[0]}/0`,
      `${scope}shared/${ids[1]}/0`,
    ]);
  });

  it("rejects file-count and total-size overflow before storing anything", async () => {
    const app = harness();
    const tooMany = Array.from({ length: 51 }, (_, i) => makeFile(`${i}.txt`, "text/plain", "x"));
    const countResponse = await app.share(tooMany);
    expect(countResponse.headers.get("location")).toBe(`${scope}?share-error=count`);
    const tooLarge = makeFile("large.bin", "application/octet-stream", "x") as Blob & { name: string; size: number; type: string };
    Object.defineProperty(tooLarge, "size", { value: 400 * 1024 * 1024 + 1 });
    const sizeResponse = await app.share([tooLarge]);
    expect(sizeResponse.headers.get("location")).toBe(`${scope}?share-error=size`);
    expect(await (await app.cacheStorage.open("hexscope-shared")).keys()).toHaveLength(0);
  });

  it("rolls back all files from a submission when a cache write hits quota", async () => {
    const app = harness();
    const inbox = await app.cacheStorage.open("hexscope-shared");
    inbox.quotaAt = 2;
    const response = await app.share([makeFile("a.txt", "text/plain", "a"), makeFile("b.txt", "text/plain", "b")]);
    expect(response.headers.get("location")).toBe(`${scope}?share-error=storage`);
    expect(await inbox.keys()).toHaveLength(0);
  });

  it("purges expired submissions and gives a legacy entry one migration grace period", async () => {
    const app = harness();
    const inbox = await app.cacheStorage.open("hexscope-shared");
    await inbox.put(
      `${scope}shared/${ids[0]}/0`,
      new Response("old", { headers: { "x-file-name": "old.txt", "x-received-at": String(app.now() - ttl - 1) } }),
    );
    await inbox.put(`${scope}shared-file`, new Response("legacy", { headers: { "x-file-name": "old.txt" } }));
    await app.dispatch("activate");
    const migrated = await inbox.match(`${scope}shared-file`);
    const migratedAt = migrated?.headers.get("x-received-at");
    expect(migratedAt).toBe(String(app.now()));
    expect(await inbox.match(`${scope}shared/${ids[0]}/0`)).toBeUndefined();
    app.advance(60 * 60 * 1000);
    await app.dispatch("activate");
    expect((await inbox.match(`${scope}shared-file`))?.headers.get("x-received-at")).toBe(migratedAt);
    app.advance(ttl);
    await app.dispatch("activate");
    expect(await inbox.match(`${scope}shared-file`)).toBeUndefined();
  });

  it("expires old records before a new share and removes data behind a consume marker", async () => {
    const app = harness();
    const inbox = await app.cacheStorage.open("hexscope-shared");
    const oldHeaders = {
      "x-submission-id": ids[0],
      "x-file-index": "0",
      "x-file-count": "1",
      "x-received-at": String(app.now() - ttl - 1),
    };
    await inbox.put(`${scope}shared/${ids[0]}/0`, new Response("expired", { headers: oldHeaders }));
    const response = await app.share([makeFile("fresh.txt", "text/plain", "fresh")]);
    expect(response.headers.get("location")).toBe(`${scope}?shared=${ids[0]}`);
    expect(await inbox.match(`${scope}shared/${ids[0]}/0`)).toBeDefined();
    expect(await inbox.match(`${scope}shared/${ids[0]}/1`)).toBeUndefined();

    await inbox.put(`${scope}shared/${ids[1]}/0`, new Response("consumed", { headers: { ...oldHeaders, "x-submission-id": ids[1], "x-received-at": String(app.now()) } }));
    await inbox.put(`${scope}shared/${ids[1]}/_consumed`, new Response("", { headers: { "x-submission-id": ids[1] } }));
    await app.dispatch("activate");
    expect(await inbox.match(`${scope}shared/${ids[1]}/0`)).toBeUndefined();
    expect(await inbox.match(`${scope}shared/${ids[1]}/_consumed`)).toBeUndefined();
  });
});
