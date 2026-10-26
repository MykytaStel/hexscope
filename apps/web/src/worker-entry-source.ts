import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";

type EntrySource = Pick<Parsed, "entryDataRange" | "extractEntry" | "extractEntryFrom" | "selectEntry" | "selectEntryFrom">;
type IsCurrent = () => boolean;

const selectionTokens = new WeakMap<EntrySource, object>();

function fileChanged(): Error {
  return new Error("the file changed while its archive entry was being read");
}

function compressedRange(parsed: EntrySource, source: Blob, index: number): [number, number] | null {
  const range = parsed.entryDataRange(index);
  if (range.length !== 2) return null;

  const [start, length] = range;
  const end = start + length;
  if (
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(length) ||
    start < 0 ||
    length < 0 ||
    !Number.isSafeInteger(end) ||
    end > source.size
  ) {
    return null;
  }
  return [start, end];
}

/** Reads and extracts a ZIP entry from the owning file only when it is external to WASM. */
export async function readEntry(
  parsed: EntrySource,
  source: Blob | null,
  index: number,
  isCurrent: IsCurrent = () => true,
): Promise<Uint8Array> {
  if (!isCurrent()) throw fileChanged();
  if (!source) return parsed.extractEntry(index);
  const range = compressedRange(parsed, source, index);
  if (!range) return parsed.extractEntry(index);
  const compressed = new Uint8Array(await source.slice(range[0], range[1]).arrayBuffer());
  if (!isCurrent()) throw fileChanged();
  return parsed.extractEntryFrom(index, compressed);
}

/** Selects a ZIP entry for the DEFLATE player, reading only its compressed range when needed. */
export async function selectEntry(
  parsed: EntrySource,
  source: Blob | null,
  index: number,
  isCurrent: IsCurrent = () => true,
): Promise<boolean> {
  const token = {};
  selectionTokens.set(parsed, token);
  const isLatest = () => selectionTokens.get(parsed) === token;

  if (!isCurrent()) return false;
  if (!source) return parsed.selectEntry(index);
  const range = compressedRange(parsed, source, index);
  if (!range) return parsed.selectEntry(index);
  const compressed = new Uint8Array(await source.slice(range[0], range[1]).arrayBuffer());
  if (!isCurrent() || !isLatest()) return false;
  return parsed.selectEntryFrom(index, compressed);
}
