import { describe, expect, it } from "vitest";
import { findAll } from "./search";

describe("byte search", () => {
  it("keeps overlapping, ASCII case-insensitive text matches", () => {
    const hay = new TextEncoder().encode("aAaAa");
    const needle = new TextEncoder().encode("AaA");

    expect(findAll(hay, needle, true)).toEqual([0, 1, 2]);
  });

  it("does not fold case in an exact byte search", () => {
    expect(findAll(new Uint8Array([0x41, 0x61, 0x41]), new Uint8Array([0x61]), false)).toEqual([1]);
  });

  it("does not rescan a long shared prefix at every byte", () => {
    const hay = new Uint8Array(250_000).fill(0x61);
    const needle = new Uint8Array(1_025).fill(0x61);
    needle[needle.length - 1] = 0x62;

    const start = performance.now();
    expect(findAll(hay, needle, false)).toEqual([]);
    expect(performance.now() - start).toBeLessThan(200);
  });
});
