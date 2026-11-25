import { describe, expect, it } from "vitest";
import { CompactByteSource } from "./compact-byte-source";
import { Missing } from "./missing";

describe("compact original-offset byte views", () => {
  it("addresses an 8 GiB logical file with only six metadata bytes", () => {
    const gap = 4 * 1024 ** 3;
    const data = Uint8Array.from([1, 2, 3, 4, 5, 6]);
    const source = new CompactByteSource(2 * gap + 6, data, [
      { at: 2, start: 2, len: gap },
      { at: 4, start: gap + 4, len: gap },
    ]);
    expect(source.view(0, 2)).toEqual(data.subarray(0, 2));
    expect(source.view(gap + 2, gap + 4)).toEqual(data.subarray(2, 4));
    expect(source.view(2 * gap + 4, 2 * gap + 6)).toEqual(data.subarray(4));
    expect(source.view(2 * gap + 4, 2 * gap + 6)?.buffer).toBe(data.buffer);
    expect(source.view(1, 3)).toBeUndefined();
    expect(source.view(gap, gap + 1)).toBeUndefined();
    expect(source.view(-1, 2)).toBeUndefined();
    expect(source.view(0, source.size + 1)).toBeUndefined();
  });
});

describe("missing-range admission", () => {
  it("checks huge missing regions without constructing a chunk list", () => {
    const size = 8 * 1024 ** 3;
    const m = new Missing(Float64Array.from([16, size - 32]));
    expect(m.hasUnread(0, 16)).toBe(false);
    expect(m.hasUnread(0, size)).toBe(true);
    expect(m.hasUnread(size - 16, size)).toBe(false);
    expect(m.available(size)).toEqual([[0, 16], [size - 16, size]]);
    expect(m.wanted(size - 16, size)).toEqual([[size - 65536, size - 16]]);
  });

  it("does not mark a short read as available", async () => {
    const m = new Missing(Float64Array.from([0, 4]));
    const source = new Blob([new Uint8Array([1, 2])]);
    expect(await m.fill(new Uint8Array(4), source, 0, 4)).toBe(false);
    expect(m.has(0)).toBe(false);
  });
});
