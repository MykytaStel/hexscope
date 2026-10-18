import { describe, expect, it } from "vitest";
import { entropy, valuesAt } from "./bytetools";
import { parseOffset } from "./search";

describe("what the bytes at a place read as", () => {
  // A PNG's signature: 89 50 4E 47 0D 0A 1A 0A.
  const png = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const rows = Object.fromEntries(valuesAt(png, 0).map(([name, le, be]) => [name, [le, be]]));

  it("as whole numbers, unsigned and signed, both ways round", () => {
    expect(rows["8-bit"]).toEqual(["137 · -119 · 0x89", ""]);
    expect(rows["16-bit"]).toEqual(["20617 · 20617", "35152 · -30384"]);
    expect(rows["32-bit"]).toEqual(["1196314761 · 1196314761", "2303741511 · -1991225785"]);
    expect(rows["64-bit"]).toEqual(["727905341920923785", "9894494448401390090"]);
    expect(rows.Bits[0]).toBe("10001001");
  });

  it("as a date only when it could be one, and as text when it starts a character", () => {
    const time = Uint8Array.from([0x00, 0x2f, 0x55, 0x68]); // 0x68552F00 read little-endian
    expect(Object.fromEntries(valuesAt(time, 0).map(([n, le, be]) => [n, [le, be]]))["Unix time"]).toEqual(["2025-06-20 09:50:56 UTC", "—"]);
    expect(Object.fromEntries(valuesAt(new TextEncoder().encode("Ї"), 0).map(([n, le]) => [n, le]))["UTF-8"]).toBe("“Ї” U+0407");
  });

  it("as far as there are bytes, and nothing past the end", () => {
    expect(valuesAt(png, 6).map(([name]) => name)).toEqual(["8-bit", "16-bit", "Bits", "UTF-8"]);
    expect(valuesAt(png, 8)).toEqual([]);
  });
});

describe("entropy along a file", () => {
  it("is 0 where one byte repeats and near 8 where all are as likely", () => {
    const flat = new Uint8Array(4096);
    const all = Uint8Array.from({ length: 4096 }, (_, i) => i % 256);
    expect([...entropy(flat, 4)]).toEqual([0, 0, 0, 0]);
    expect(entropy(all, 1)[0]).toBeCloseTo(8, 5);
    expect(entropy(new Uint8Array(0), 10).length).toBe(1);
  });
});

describe("going to an offset from the search box", () => {
  it("takes @ and a hex or decimal offset, and nothing else", () => {
    expect(parseOffset("@0x1F0")).toBe(0x1f0);
    expect(parseOffset(" @ 496 ")).toBe(496);
    expect(parseOffset("0x1F0")).toBeNull();
    expect(parseOffset("@here")).toBeNull();
  });
});
