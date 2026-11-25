import { describe, expect, it } from "vitest";
import { EMBEDDED_READ_BYTES, findEmbedded, findEmbeddedInBlob } from "./embedded";

const text = (value: string) => new TextEncoder().encode(value);

describe("streaming embedded signature search", () => {
  it("matches contiguous results for headers and end markers across windows", async () => {
    const page = EMBEDDED_READ_BYTES;
    const data = new Uint8Array(4 * page + 128);
    data.set(text("GIF89a"), page - 2);
    data.set(Uint8Array.of(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a), page + 64);
    data.set(text("IEND"), 2 * page - 2);
    data.set(Uint8Array.of(0x50, 0x4b, 0x03, 0x04), 2 * page + 64);
    data.set(Uint8Array.of(0x50, 0x4b, 0x05, 0x06), 3 * page - 2);
    data.set(text("%PDF-"), 3 * page + 64);
    data.set(text("%%EOF"), 4 * page - 2);
    const blob = new Blob([data]);
    let maxRead = 0;
    const source = {
      size: blob.size,
      slice(start: number, end: number) {
        maxRead = Math.max(maxRead, end - start);
        return blob.slice(start, end);
      },
    } as unknown as Blob;
    const ranges: [number, number][] = [[0, data.length]];
    const expected = await findEmbedded(data, ranges);
    const result = await findEmbeddedInBlob(source, ranges);
    expect(result).toEqual(expected);
    expect(result.matches.map((m) => m.ext)).toEqual(["gif", "png", "zip", "pdf"]);
    expect(maxRead).toBeLessThanOrEqual(page);
  });

  it("bounds physical reads by the logical search budget", async () => {
    const ranges: [number, number][] = [[100, 1024 * 1024 * 1024]];
    const source = {
      size: ranges[0][1],
      slice(start: number, end: number) {
        expect(start).toBeGreaterThanOrEqual(100);
        expect(end).toBeLessThanOrEqual(112);
        return new Blob([new Uint8Array(end - start)]);
      },
    } as unknown as Blob;
    const result = await findEmbeddedInBlob(source, ranges, [], { maxBytes: 12 });
    expect(result.searched).toBe(12);
    expect(result.limited).toBe(true);
  });

  it("preserves offset zero exclusion and does not validate a truncated header", async () => {
    const bytes = text("GIF89aRIFF0000WEBP");
    const ranges: [number, number][] = [[0, 10]];
    const expected = await findEmbedded(bytes, ranges);
    expect(expected.matches).toEqual([]);
    expect(await findEmbeddedInBlob(new Blob([bytes]), ranges)).toEqual(expected);
  });

  it("rejects short reads and cancellation during a read", async () => {
    const short = { size: 10, slice: () => new Blob(["x"]) } as unknown as Blob;
    await expect(findEmbeddedInBlob(short, [[0, 10]])).rejects.toThrow(/read/i);
    const controller = new AbortController();
    const cancelled = {
      size: 10,
      slice() {
        controller.abort();
        return new Blob([new Uint8Array(10)]);
      },
    } as unknown as Blob;
    await expect(findEmbeddedInBlob(cancelled, [[0, 10]], [], { signal: controller.signal })).rejects.toMatchObject({ name: "AbortError" });
  });
});
