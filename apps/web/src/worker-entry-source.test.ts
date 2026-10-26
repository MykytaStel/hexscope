import { describe, expect, it, vi } from "vitest";
import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";
import { readEntry, selectEntry } from "./worker-entry-source";

describe("external ZIP source ranges", () => {
  it("reads only the compressed range before asking WASM to open an entry", async () => {
    const parsed = {
      entryDataRange: () => new Float64Array([2, 4]),
      extractEntry: vi.fn(() => new Uint8Array()),
      extractEntryFrom: vi.fn(() => new Uint8Array([7, 8])),
    } as unknown as Parsed;

    await expect(readEntry(parsed, new Blob(["0123456789"]), 3)).resolves.toEqual(new Uint8Array([7, 8]));
    expect(parsed.extractEntryFrom).toHaveBeenCalledWith(3, new Uint8Array([50, 51, 52, 53]));
    expect(parsed.extractEntry).not.toHaveBeenCalled();
  });

  it("keeps nested archives on their retained WASM source", async () => {
    const opened = new Uint8Array([1, 2]);
    const parsed = {
      extractEntry: vi.fn(() => opened),
    } as unknown as Parsed;

    await expect(readEntry(parsed, null, 0)).resolves.toBe(opened);
    expect(parsed.extractEntry).toHaveBeenCalledWith(0);
  });

  it("does not use a parsed file after its source changes during the range read", async () => {
    const parsed = {
      entryDataRange: () => new Float64Array([0, 3]),
      extractEntry: vi.fn(() => new Uint8Array()),
      extractEntryFrom: vi.fn(() => new Uint8Array([1])),
    } as unknown as Parsed;

    await expect(readEntry(parsed, new Blob(["abcdef"]), 0, () => false)).rejects.toThrow("file changed");
    expect(parsed.extractEntryFrom).not.toHaveBeenCalled();
  });

  it("reads only the selected compressed range for the DEFLATE player", async () => {
    const parsed = {
      entryDataRange: () => new Float64Array([1, 3]),
      selectEntry: vi.fn(() => false),
      selectEntryFrom: vi.fn(() => true),
    } as unknown as Parsed;

    await expect(selectEntry(parsed, new Blob(["abcdef"]), 2)).resolves.toBe(true);
    expect(parsed.selectEntryFrom).toHaveBeenCalledWith(2, new Uint8Array([98, 99, 100]));
    expect(parsed.selectEntry).not.toHaveBeenCalled();
  });

  it("does not select an entry after its parsed file changes during the range read", async () => {
    const parsed = {
      entryDataRange: () => new Float64Array([0, 3]),
      selectEntry: vi.fn(() => false),
      selectEntryFrom: vi.fn(() => true),
    } as unknown as Parsed;

    await expect(selectEntry(parsed, new Blob(["abcdef"]), 0, () => false)).rejects.toThrow("file changed");
    expect(parsed.selectEntryFrom).not.toHaveBeenCalled();
  });
});
