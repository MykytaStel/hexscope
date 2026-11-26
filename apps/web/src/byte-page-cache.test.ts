import { describe, expect, it } from "vitest";
import { BYTE_CACHE_PAGES, BYTE_PAGE_SIZE, BytePageCache } from "./byte-page-cache";

describe("bounded media cache", () => {
  it("uses original offsets without allocating a logical 8 GiB file", async () => {
    const size = 8 * 1024 ** 3;
    let reads = 0;
    const source = {
      size,
      slice(start: number, end: number) {
        reads++;
        expect(end - start).toBeLessThanOrEqual(BYTE_PAGE_SIZE);
        return new Blob([new Uint8Array(end - start).fill(7)]);
      },
    } as unknown as Blob;
    const cache = new BytePageCache(source);
    expect(cache.byteAt(size - 1)).toBeUndefined();
    await Promise.all([cache.load(size - 2, size), cache.load(size - 1, size)]);
    expect(reads).toBe(1);
    expect(cache.byteAt(size - 1)).toBe(7);
    for (let i = 0; i < BYTE_CACHE_PAGES; i++) {
      await cache.load(i * BYTE_PAGE_SIZE, i * BYTE_PAGE_SIZE + 1);
    }
    expect(cache.retainedBytes).toBeLessThanOrEqual(BYTE_CACHE_PAGES * BYTE_PAGE_SIZE);
    expect(cache.byteAt(size - 1)).toBeUndefined();
    expect(cache.byteAt(0)).toBe(7);
  });

  it("does not publish short reads", async () => {
    const source = { size: 10, slice: () => new Blob(["a"]) } as unknown as Blob;
    const cache = new BytePageCache(source);
    expect(await cache.load(0, 10)).toBe(false);
    expect(cache.byteAt(0)).toBeUndefined();
    expect(cache.retainedBytes).toBe(0);
  });
});

describe("adaptive cache limits", () => {
  it("retains only the requested number of pages and preserves recency", async () => {
    const source = new Blob([new Uint8Array(5 * BYTE_PAGE_SIZE).fill(9)]);
    const cache = new BytePageCache(source, 2, 1);
    await cache.load(0, BYTE_PAGE_SIZE * 3);
    expect(cache.retainedBytes).toBe(BYTE_PAGE_SIZE);
    expect(cache.byteAt(BYTE_PAGE_SIZE)).toBeUndefined();
    await cache.load(BYTE_PAGE_SIZE, BYTE_PAGE_SIZE + 1);
    expect(cache.byteAt(0)).toBe(9);
    expect(cache.byteAt(1)).toBe(9);
    await cache.load(2 * BYTE_PAGE_SIZE, 2 * BYTE_PAGE_SIZE + 1);
    expect(cache.retainedBytes).toBe(2 * BYTE_PAGE_SIZE);
    expect(cache.byteAt(BYTE_PAGE_SIZE)).toBeUndefined();
    expect(cache.byteAt(0)).toBe(9);
  });

  it("rejects invalid budgets", () => {
    const source = new Blob();
    expect(() => new BytePageCache(source, 0, 1)).toThrow();
    expect(() => new BytePageCache(source, 2, 3)).toThrow();
    expect(() => new BytePageCache(source, 65, 1)).toThrow();
  });
});

it("clears retained pages and prevents late reads from repopulating them", async () => {
  let finish!: (value: ArrayBuffer) => void;
  let slow = false;
  const source = {
    size: 2 * BYTE_PAGE_SIZE,
    slice(start: number, end: number) {
      return {
        arrayBuffer: () => slow
          ? new Promise<ArrayBuffer>(resolve => { finish = resolve; })
          : Promise.resolve(new Uint8Array(end - start).fill(7).buffer),
      };
    },
  } as unknown as Blob;
  const cache = new BytePageCache(source, 2, 1);
  await cache.load(0, 1);
  expect(cache.retainedBytes).toBe(BYTE_PAGE_SIZE);
  slow = true;
  const pending = cache.load(BYTE_PAGE_SIZE, BYTE_PAGE_SIZE + 1);
  await Promise.resolve();
  cache.clear();
  expect(cache.retainedBytes).toBe(0);
  expect(await cache.load(0, 1)).toBe(false);
  finish(new Uint8Array(BYTE_PAGE_SIZE).fill(8).buffer);
  expect(await pending).toBe(false);
  expect(cache.byteAt(BYTE_PAGE_SIZE)).toBeUndefined();
  slow = false;
  expect(await cache.load(BYTE_PAGE_SIZE, BYTE_PAGE_SIZE + 1)).toBe(true);
  expect(cache.byteAt(BYTE_PAGE_SIZE)).toBe(7);
});
