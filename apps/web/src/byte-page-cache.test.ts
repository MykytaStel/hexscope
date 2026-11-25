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
    } as Blob;
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
