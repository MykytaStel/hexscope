import { describe, expect, it, vi } from "vitest";
import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";
import { readEntry, selectEntry } from "./worker-entry-source";

function delayedSource(): { blob: Blob; resolve: (start: number, bytes: number[]) => void } {
  const reads = new Map<number, (buffer: ArrayBuffer) => void>();
  const blob = {
    size: 8,
    slice(start: number) {
      return { arrayBuffer: () => new Promise<ArrayBuffer>((resolve) => reads.set(start, resolve)) };
    },
  } as unknown as Blob;
  return {
    blob,
    resolve(start, bytes) {
      const resolve = reads.get(start);
      if (!resolve) throw new Error(`no pending read at ${start}`);
      resolve(new Uint8Array(bytes).buffer);
    },
  };
}

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

  it("does not extract a nested entry from a freed parsed file", async () => {
    const parsed = {
      entryDataRange: vi.fn(() => new Float64Array([0, 3])),
      extractEntry: vi.fn(() => new Uint8Array()),
      extractEntryFrom: vi.fn(() => new Uint8Array()),
    } as unknown as Parsed;

    await expect(readEntry(parsed, null, 0, () => false)).rejects.toThrow("file changed");
    expect(parsed.entryDataRange).not.toHaveBeenCalled();
    expect(parsed.extractEntry).not.toHaveBeenCalled();
    expect(parsed.extractEntryFrom).not.toHaveBeenCalled();
  });

  it("does not call WASM when the parsed file is already stale", async () => {
    const parsed = {
      entryDataRange: vi.fn(() => new Float64Array([0, 3])),
      extractEntry: vi.fn(() => new Uint8Array()),
      extractEntryFrom: vi.fn(() => new Uint8Array([1])),
    } as unknown as Parsed;

    await expect(readEntry(parsed, new Blob(["abcdef"]), 0, () => false)).rejects.toThrow("file changed");
    expect(parsed.entryDataRange).not.toHaveBeenCalled();
    expect(parsed.extractEntry).not.toHaveBeenCalled();
    expect(parsed.extractEntryFrom).not.toHaveBeenCalled();
  });

  it("checks file identity again after the compressed range read", async () => {
    const source = delayedSource();
    const parsed = {
      entryDataRange: () => new Float64Array([0, 3]),
      extractEntry: vi.fn(() => new Uint8Array()),
      extractEntryFrom: vi.fn(() => new Uint8Array([1])),
    } as unknown as Parsed;
    let current = true;

    const pending = readEntry(parsed, source.blob, 0, () => current);
    current = false;
    source.resolve(0, [1, 2, 3]);

    await expect(pending).rejects.toThrow("file changed");
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

  it("does not select a nested entry from a freed parsed file", async () => {
    const parsed = {
      entryDataRange: vi.fn(() => new Float64Array([0, 3])),
      selectEntry: vi.fn(() => false),
      selectEntryFrom: vi.fn(() => true),
    } as unknown as Parsed;

    await expect(selectEntry(parsed, null, 0, () => false)).resolves.toBe(false);
    expect(parsed.entryDataRange).not.toHaveBeenCalled();
    expect(parsed.selectEntry).not.toHaveBeenCalled();
    expect(parsed.selectEntryFrom).not.toHaveBeenCalled();
  });

  it("does not query WASM if the selected file is already stale", async () => {
    const source = delayedSource();
    const parsed = {
      entryDataRange: vi.fn(() => new Float64Array([0, 3])),
      selectEntry: vi.fn(() => false),
      selectEntryFrom: vi.fn(() => true),
    } as unknown as Parsed;

    await expect(selectEntry(parsed, source.blob, 0, () => false)).resolves.toBe(false);
    expect(parsed.entryDataRange).not.toHaveBeenCalled();
    expect(parsed.selectEntry).not.toHaveBeenCalled();
    expect(parsed.selectEntryFrom).not.toHaveBeenCalled();
  });

  it("does not select an entry after its parsed file changes during the range read", async () => {
    const source = delayedSource();
    const parsed = {
      entryDataRange: () => new Float64Array([0, 3]),
      selectEntry: vi.fn(() => false),
      selectEntryFrom: vi.fn(() => true),
    } as unknown as Parsed;
    let current = true;

    const pending = selectEntry(parsed, source.blob, 0, () => current);
    current = false;
    source.resolve(0, [1, 2, 3]);

    await expect(pending).resolves.toBe(false);
    expect(parsed.selectEntryFrom).not.toHaveBeenCalled();
    expect(parsed.selectEntry).not.toHaveBeenCalled();
  });

  it("does not let an older selection overwrite the newest ZIP entry", async () => {
    const source = delayedSource();
    const parsed = {
      entryDataRange: (index: number) => new Float64Array([index * 2, 1]),
      selectEntry: vi.fn(() => false),
      selectEntryFrom: vi.fn(() => true),
    } as unknown as Parsed;

    const older = selectEntry(parsed, source.blob, 0);
    const newer = selectEntry(parsed, source.blob, 1);

    source.resolve(2, [22]);
    await expect(newer).resolves.toBe(true);
    source.resolve(0, [11]);
    await expect(older).resolves.toBe(false);
    expect(parsed.selectEntryFrom).toHaveBeenCalledOnce();
    expect(parsed.selectEntryFrom).toHaveBeenCalledWith(1, new Uint8Array([22]));
  });
});
