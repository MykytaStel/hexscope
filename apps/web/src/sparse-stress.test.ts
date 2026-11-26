import { describe, expect, it } from "vitest";
import { readMovie, movieEntropy } from "./movie";
import { BytePageCache, BYTE_PAGE_SIZE } from "./byte-page-cache";

// A virtual 8 GiB source: every read is accounted for and only the requested
// bytes exist. Extended-size mdat is followed by a small known metadata box.
const GiB = 1024 ** 3;
const mediaStart = 28;
const mediaEnd = mediaStart + 8 * GiB;
const size = mediaEnd + 12;
function virtualMovie() {
  const head = new Uint8Array(mediaStart);
  const view = new DataView(head.buffer);
  view.setUint32(0, 12);
  head.set([102, 116, 121, 112, 105, 115, 111, 109], 4);
  view.setUint32(12, 1);
  head.set([109, 100, 97, 116], 16);
  view.setBigUint64(20, BigInt(8 * GiB + 16));
  const tail = Uint8Array.of(0, 0, 0, 12, 102, 114, 101, 101, 11, 22, 33, 44);
  let total = 0, active = 0, peak = 0, calls = 0;
  const source = {
    size,
    slice(start: number, end: number) {
      expect(Number.isSafeInteger(start) && Number.isSafeInteger(end)).toBe(true);
      expect(start).toBeGreaterThanOrEqual(0);
      expect(end).toBeLessThanOrEqual(size);
      expect(end - start).toBeLessThanOrEqual(BYTE_PAGE_SIZE);
      calls++;
      return {
        async arrayBuffer() {
          active++;
          peak = Math.max(peak, active);
          total += end - start;
          try {
            await Promise.resolve();
            const out = new Uint8Array(end - start);
            for (let i = 0; i < out.length; i++) {
              const at = start + i;
              out[i] = at < mediaStart ? head[at] : at >= mediaEnd ? tail[at - mediaEnd] : at % 251;
            }
            return out.buffer;
          } finally { active--; }
        },
      };
    },
  } as unknown as Blob;
  return { source, stats: () => ({ total, peak, calls }) };
}

describe("logical large-file resource contracts", () => {
  it("reads 8 GiB of logical movie structure and entropy without allocating the media", async () => {
    const { source, stats } = virtualMovie();
    const movie = await readMovie(source, () => true, 0, { includeWhole: false, maxCompactBytes: 64 });
    expect(movie).not.toBeNull();
    expect(movie!.given.length).toBe(40);
    expect(movie!.given.buffer.byteLength).toBe(40);
    expect(movie!.restoreWhole()).toBeNull();
    expect(Array.from(movie!.missing)).toEqual([mediaStart, 8 * GiB]);
    expect(movie!.knownBytes!(mediaEnd + 8, size)).toEqual(Uint8Array.of(11, 22, 33, 44));
    expect(movie!.knownBytes!(mediaEnd - 1, mediaEnd + 1)).toBeUndefined();
    expect(stats().total).toBeLessThan(128);
    const before = stats().total;
    const result = await movieEntropy(source, movie!, 256, bytes => bytes.length ? 1 : 0);
    expect(result.values.length).toBe(256);
    expect(result.values.every(value => value === 1)).toBe(true);
    expect(stats().total - before).toBeLessThanOrEqual(256 * 4096);
    expect(stats().peak).toBeLessThanOrEqual(8);
  });

  it("rejects an insufficient compact budget before reading metadata ranges", async () => {
    const { source, stats } = virtualMovie();
    await expect(readMovie(source, () => true, 0, { includeWhole: false, maxCompactBytes: 39 }))
      .rejects.toThrow("too much non-media data");
    expect(stats().total).toBeLessThan(64);
  });

  it("bounds concurrent and retained pages across repeated disjoint far offsets", async () => {
    const { source, stats } = virtualMovie();
    const cache = new BytePageCache(source, 16, 2);
    for (let round = 0; round < 32; round++) {
      const at = GiB + round * 128 * 1024 ** 2;
      await Promise.all([
        cache.load(at, at + BYTE_PAGE_SIZE * 10),
        cache.load(at, at + 1),
        cache.load(at + GiB, at + GiB + 1),
      ]);
      expect(cache.byteAt(at)).toBe(at % 251);
      expect(cache.retainedBytes).toBeLessThanOrEqual(16 * BYTE_PAGE_SIZE);
    }
    expect(stats().peak).toBeLessThanOrEqual(2);
    expect(stats().calls).toBe(64);
    expect(stats().total).toBe(64 * BYTE_PAGE_SIZE);
    cache.clear();
    expect(cache.retainedBytes).toBe(0);
  });
});
