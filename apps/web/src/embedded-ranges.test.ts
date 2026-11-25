import { describe, expect, it } from "vitest";
import { findEmbeddedRanges } from "./embedded";

describe("embedded signatures in original-offset views", () => {
  it("retains original offsets and does not discard a signature at a range start", async () => {
    const bytes = new TextEncoder().encode("GIF89a");
    const result = await findEmbeddedRanges([[100, 106], [1000, 1006]], () => bytes);
    expect(result.matches.map(({ start, end }) => [start, end])).toEqual([[100, 106], [1000, 1006]]);
    expect(result.searched).toBe(12);
    expect(result.limited).toBe(false);
  });

  it("respects the global byte budget and reports unread ranges", async () => {
    const bytes = new TextEncoder().encode("GIF89a");
    const result = await findEmbeddedRanges([[100, 106], [1000, 1006]], (start, end) => bytes.subarray(0, end - start), { maxBytes: 6 });
    expect(result.matches).toHaveLength(1);
    expect(result.searched).toBe(6);
    expect(result.limited).toBe(true);
  });
});
