import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";
import { MAX_INPUT_BYTES } from "./resource-budget";

/** Bound the sum of external compressed ranges in flight, not WASM heap. */
let reservedCompressedBytes = 0;

async function withCompressedRange<T>(
  source: Blob,
  start: number,
  end: number,
  use: (bytes: Uint8Array) => T,
): Promise<T> {
  const length = end - start;
  if (length > MAX_INPUT_BYTES - reservedCompressedBytes) {
    throw new Error("Archive entry reading exceeds the browser's compressed-byte budget. Try again after the other reads finish.");
  }
  reservedCompressedBytes += length;
  try {
    const bytes = new Uint8Array(await source.slice(start, end).arrayBuffer());
    if (bytes.length !== length) throw new Error("The archive entry's compressed byte range was not read completely.");
    return use(bytes);
  } finally {
    reservedCompressedBytes -= length;
  }
}

type EntrySource = Pick<Parsed, "entryDataRange" | "extractEntry" | "extractEntryFrom" | "selectEntry" | "selectEntryFrom">;
type IsCurrent = () => boolean;

const selectionTokens = new WeakMap<EntrySource, object>();

function fileChanged(): Error {
  return new Error("the file changed while its archive entry was being read");
}

function compressedRange(parsed: EntrySource, source: Blob, index: number): [number, number] | null {
  const range = parsed.entryDataRange(index);
  // An empty range is the documented WASM fallback for an unavailable range.
  if (range.length === 0) return null;
  if (range.length !== 2) throw new Error("Invalid archive entry byte range.");

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
    throw new Error("Invalid archive entry byte range.");
  }
  return [start, end];
}

/** Reads and extracts a ZIP entry from the owning file only when it is external to WASM. */
export async function readEntry(
  parsed: EntrySource,
  source: Blob | null,
  index: number,
  isCurrent: IsCurrent = () => true,
  signal?: AbortSignal,
): Promise<Uint8Array> {
  signal?.throwIfAborted();
  if (!isCurrent()) throw fileChanged();
  if (!source) return parsed.extractEntry(index);
  const range = compressedRange(parsed, source, index);
  if (!range) return parsed.extractEntry(index);
  return withCompressedRange(source, range[0], range[1], compressed => {
    signal?.throwIfAborted();
    if (!isCurrent()) throw fileChanged();
    return parsed.extractEntryFrom(index, compressed);
  });
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
  return withCompressedRange(source, range[0], range[1], compressed => {
    if (!isCurrent() || !isLatest()) return false;
    return parsed.selectEntryFrom(index, compressed);
  });
}
