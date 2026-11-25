// Files inside files: where another file's signature appears in the bytes,
// what it is, and how far it goes — a picture tacked onto the end of
// another, a ZIP behind a JPEG, the photos inside a PDF, the thumbnail in a
// camera's EXIF. Found by signature, the way binwalk finds them, and opened
// with the same parsers as any file.

export interface Embedded {
  start: number;
  end: number;
  /** "a JPEG picture" */
  what: string;
  /** The extension a saved copy gets. */
  ext: string;
}

export interface EmbeddedSearchProgress {
  /** Bytes examined so far, including bytes checked while finding file ends. */
  searched: number;
  /** The byte budget for this search. */
  total: number;
  matches: number;
}

export interface EmbeddedSearchResult {
  matches: Embedded[];
  searched: number;
  /** Bytes available in the requested ranges. */
  available: number;
  /** The byte budget or match limit stopped the search early. */
  limited: boolean;
  matchLimitReached: boolean;
}

export interface EmbeddedSearchOptions {
  signal?: AbortSignal;
  onProgress?: (progress: EmbeddedSearchProgress) => void;
  /** A lower per-search cap, used for background discovery. */
  maxBytes?: number;
  /** Original offset of this buffer, for a bounded range view. */
  baseOffset?: number;
}

/** Files listed, at most. */
const MAX_FOUND = 100;
/** Bytes searched, at most: past this, the rest is not looked at. */
const MAX_SEARCHED = 256 * 1024 * 1024;
/** Bound uninterrupted work so a large file cannot monopolize the UI thread. */
const YIELD_EVERY = 256 * 1024;
/** One read window plus a small look-behind/look-ahead at each edge. */
export const EMBEDDED_READ_BYTES = 1024 * 1024;
const LOOK_AROUND = 32;

interface ScanBytes {
  readonly length: number;
  at(offset: number): number;
  covers(start: number, end: number): boolean;
  load(start: number, end: number): Promise<void>;
  setRange(from: number, limit: number): void;
}

function contiguous(bytes: Uint8Array): ScanBytes {
  return {
    length: bytes.length,
    at: (offset) => bytes[offset],
    covers: () => true,
    load: async () => {},
    setRange: () => {},
  };
}

/** A single reusable read window; never materialize the entire search prefix. */
function paged(source: Blob, signal?: AbortSignal): ScanBytes {
  let start = 0;
  let data = new Uint8Array(0);
  let rangeFrom = 0;
  let rangeLimit = source.size;
  return {
    length: source.size,
    at: (offset) => offset >= rangeFrom && offset < rangeLimit ? data[offset - start] : NaN,
    setRange(from, limit) {
      rangeFrom = from;
      rangeLimit = limit;
      data = new Uint8Array(0);
    },
    covers: (from, to) => from >= start && to <= start + data.length,
    async load(from, to) {
      signal?.throwIfAborted();
      const next = Math.max(rangeFrom, from - LOOK_AROUND);
      const end = Math.min(rangeLimit, Math.max(to, next + EMBEDDED_READ_BYTES));
      // Release the previous window before issuing the next read.
      data = new Uint8Array(0);
      const bytes = new Uint8Array(await source.slice(next, end).arrayBuffer());
      signal?.throwIfAborted();
      if (bytes.length !== end - next) throw new Error("Could not read the file's media bytes.");
      start = next;
      data = bytes;
    },
  };
}

function covered(b: ScanBytes, start: number, end: number, from: number, limit: number): boolean {
  return b.covers(Math.max(from, start), Math.min(limit, end));
}

const ascii = (s: string) => Uint8Array.from(s, (c) => c.charCodeAt(0));
type Checkpoint = (offset: number) => Promise<void>;

interface Signature {
  magic: Uint8Array;
  /** Where the magic sits from the file's start. */
  at?: number;
  what: string;
  ext: string;
  /** More than the magic that must hold, so noise is not taken for a file. */
  ok?: (b: ScanBytes, i: number) => boolean;
  /** Minimum available header bytes for the validator. */
  headerBytes?: number;
  /** Where the file ends, when its format says; otherwise it runs to the next one found. */
  end?: (b: ScanBytes, i: number, limit: number, checkpoint: Checkpoint) => Promise<number>;
}

const be32 = (b: ScanBytes, i: number) => ((b.at(i) << 24) | (b.at(i + 1) << 16) | (b.at(i + 2) << 8) | b.at(i + 3)) >>> 0;
const le16 = (b: ScanBytes, i: number) => b.at(i) | (b.at(i + 1) << 8);

async function indexOf(b: ScanBytes, needle: Uint8Array, from: number, limit: number, checkpoint: Checkpoint): Promise<number> {
  let nextYield = from + YIELD_EVERY;
  outer: for (let i = from; i + needle.length <= limit; i++) {
    if (i >= nextYield) {
      await checkpoint(i);
      nextYield = i + YIELD_EVERY;
    }
    if (!b.covers(i, i + needle.length)) await b.load(i, Math.min(limit, i + needle.length));
    for (let k = 0; k < needle.length; k++) if (b.at(i + k) !== needle[k]) continue outer;
    return i;
  }
  return -1;
}

const IEND = ascii("IEND");
const EOCD = Uint8Array.of(0x50, 0x4b, 0x05, 0x06);
const EOF = ascii("%%EOF");

const SIGNATURES: Signature[] = [
  {
    magic: Uint8Array.of(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a),
    what: "a PNG picture",
    ext: "png",
    end: async (b, i, limit, checkpoint) => {
      const at = await indexOf(b, IEND, i + 8, limit, checkpoint);
      return at < 0 ? -1 : Math.min(limit, at + 8);
    },
  },
  {
    magic: Uint8Array.of(0xff, 0xd8, 0xff),
    what: "a JPEG picture",
    ext: "jpg",
    headerBytes: 4,
    ok: (b, i) => (b.at(i + 3) >= 0xe0 && b.at(i + 3) <= 0xef) || b.at(i + 3) === 0xdb || b.at(i + 3) === 0xc0 || b.at(i + 3) === 0xc4 || b.at(i + 3) === 0xfe,
    // Segment by segment to the first scan — a thumbnail inside is skipped
    // whole — then to the end marker, which scan data never holds.
    end: async (b, i, limit, checkpoint) => {
      let k = i + 2;
      let checkpointAt = i + YIELD_EVERY;
      while (k + 4 <= limit) {
        if (!b.covers(k, k + 4)) await b.load(k, k + 4);
        if (b.at(k) !== 0xff) break;
        if (k >= checkpointAt) {
          await checkpoint(k);
          checkpointAt = k + YIELD_EVERY;
        }
        const marker = b.at(k + 1);
        if (marker === 0xda) break;
        if (marker === 0xd8 || (marker >= 0xd0 && marker <= 0xd7) || marker === 0x01 || marker === 0xff) {
          k += marker === 0xff ? 1 : 2;
          continue;
        }
        k += 2 + ((b.at(k + 2) << 8) | b.at(k + 3));
      }
      for (; k + 1 < limit; k++) {
        if (k >= checkpointAt) {
          await checkpoint(k);
          checkpointAt = k + YIELD_EVERY;
        }
        if (!b.covers(k, k + 2)) await b.load(k, k + 2);
        if (b.at(k) === 0xff && b.at(k + 1) === 0xd9) return k + 2;
      }
      return -1;
    },
  },
  { magic: ascii("GIF87a"), what: "a GIF picture", ext: "gif" },
  { magic: ascii("GIF89a"), what: "a GIF picture", ext: "gif" },
  {
    magic: ascii("RIFF"),
    what: "a WebP picture",
    ext: "webp",
    headerBytes: 12,
    ok: (b, i) => b.at(i + 8) === 0x57 && b.at(i + 9) === 0x45 && b.at(i + 10) === 0x42 && b.at(i + 11) === 0x50,
    end: async (b, i, limit) => Math.min(limit, i + 8 + (b.at(i + 4) | (b.at(i + 5) << 8) | (b.at(i + 6) << 16) | (b.at(i + 7) << 24))),
  },
  { magic: ascii("RIFF"), what: "a WAV recording", ext: "wav", headerBytes: 12, ok: (b, i) => b.at(i + 8) === 0x57 && b.at(i + 9) === 0x41 && b.at(i + 10) === 0x56 && b.at(i + 11) === 0x45 },
  { magic: ascii("RIFX"), what: "a big-endian RIFX/WAVE recording", ext: "wav", headerBytes: 12, ok: (b, i) => b.at(i + 8) === 0x57 && b.at(i + 9) === 0x41 && b.at(i + 10) === 0x56 && b.at(i + 11) === 0x45 },
  { magic: ascii("RF64"), what: "an RF64 WAVE recording", ext: "wav", headerBytes: 12, ok: (b, i) => b.at(i + 8) === 0x57 && b.at(i + 9) === 0x41 && b.at(i + 10) === 0x56 && b.at(i + 11) === 0x45 },
  { magic: ascii("BW64"), what: "a BW64 WAVE recording", ext: "wav", headerBytes: 12, ok: (b, i) => b.at(i + 8) === 0x57 && b.at(i + 9) === 0x41 && b.at(i + 10) === 0x56 && b.at(i + 11) === 0x45 },
  {
    magic: Uint8Array.of(0x50, 0x4b, 0x03, 0x04),
    what: "a ZIP archive",
    ext: "zip",
    end: async (b, i, limit, checkpoint) => {
      const at = await indexOf(b, EOCD, i + 4, limit, checkpoint);
      if (at < 0 || at + 22 > limit) return -1;
      if (!b.covers(at, at + 22)) await b.load(at, at + 22);
      return Math.min(limit, at + 22 + le16(b, at + 20));
    },
  },
  {
    magic: ascii("%PDF-"),
    what: "a PDF document",
    ext: "pdf",
    end: async (b, i, limit, checkpoint) => {
      // The last end-of-file marker before the limit: updates add more.
      let last = -1;
      for (let at = await indexOf(b, EOF, i, limit, checkpoint); at >= 0; at = await indexOf(b, EOF, at + 5, limit, checkpoint)) last = at + 5;
      return last;
    },
  },
  {
    magic: ascii("ftyp"),
    at: 4,
    what: "an MP4, HEIC or QuickTime file",
    ext: "mp4",
    headerBytes: 12,
    ok: (b, i) => {
      const size = be32(b, i);
      return size >= 16 && size <= 256 && /^[\x20-\x7e]{4}$/.test(String.fromCharCode(b.at(i + 8), b.at(i + 9), b.at(i + 10), b.at(i + 11)));
    },
  },
  { magic: Uint8Array.of(0x1f, 0x8b, 0x08), what: "a gzip archive", ext: "gz" },
  { magic: Uint8Array.of(0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c), what: "a 7-Zip archive", ext: "7z" },
  { magic: ascii("Rar!\x1a\x07"), what: "a RAR archive", ext: "rar" },
  { magic: Uint8Array.of(0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00), what: "a WebAssembly module", ext: "wasm" },
  { magic: Uint8Array.of(0x7f, 0x45, 0x4c, 0x46), what: "an ELF program", ext: "elf", headerBytes: 5, ok: (b, i) => b.at(i + 4) === 1 || b.at(i + 4) === 2 },
  { magic: ascii("OggS\x00"), what: "an Ogg audio or video file", ext: "ogg" },
  { magic: ascii("ID3"), what: "an MP3 file", ext: "mp3", headerBytes: 6, ok: (b, i) => b.at(i + 3) >= 2 && b.at(i + 3) <= 4 && b.at(i + 5) < 0x80 },
];

/** Signatures by first byte, for a single pass. */
const BY_FIRST = new Map<number, Signature[]>();
for (const s of SIGNATURES) {
  const first = s.magic[0];
  BY_FIRST.set(first, [...(BY_FIRST.get(first) ?? []), s]);
}

/**
 * Files whose signature starts within the requested ranges. The scan yields
 * at short intervals and can be cancelled. `maxBytes` is a total budget over
 * all ranges, not a fresh allowance for each range.
 */
export function findEmbedded(
  bytes: Uint8Array,
  ranges: readonly [number, number][],
  skip: readonly string[] = [],
  options: EmbeddedSearchOptions = {},
): Promise<EmbeddedSearchResult> {
  return scanEmbedded(contiguous(bytes), ranges, skip, options);
}

/** Scan the same signatures directly from a Blob, without allocating its prefix. */
export function findEmbeddedInBlob(
  source: Blob,
  ranges: readonly [number, number][],
  skip: readonly string[] = [],
  options: EmbeddedSearchOptions = {},
): Promise<EmbeddedSearchResult> {
  return scanEmbedded(paged(source, options.signal), ranges, skip, options);
}

async function scanEmbedded(
  bytes: ScanBytes,
  ranges: readonly [number, number][],
  skip: readonly string[],
  options: EmbeddedSearchOptions,
): Promise<EmbeddedSearchResult> {
  const normalized = ranges
    .map(([from, to]): [number, number] => [Math.max(0, Math.min(bytes.length, Math.trunc(from))), Math.max(0, Math.min(bytes.length, Math.trunc(to)))])
    .filter(([from, to]) => from < to);
  const available = normalized.reduce((n, [from, to]) => n + to - from, 0);
  const budget = Math.max(0, Math.min(MAX_SEARCHED, Math.trunc(options.maxBytes ?? MAX_SEARCHED)));
  const total = Math.min(available, budget);
  const out: Embedded[] = [];
  const skipped = new Set(skip);
  let searched = 0;
  let nextYield = 0;
  let activeRangeFrom = 0;
  let rangeSearched = 0;

  const checkpoint: Checkpoint = async (offset) => {
    if (options.signal?.aborted) throw new DOMException("Embedded file search cancelled", "AbortError");
    if (offset < nextYield) return;
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    if (options.signal?.aborted) throw new DOMException("Embedded file search cancelled", "AbortError");
    nextYield = offset + YIELD_EVERY;
    rangeSearched = Math.max(rangeSearched, offset - activeRangeFrom + 1);
    options.onProgress?.({ searched: Math.min(total, searched + rangeSearched), total, matches: out.length });
  };

  for (const [from, to] of normalized) {
    const remaining = budget - searched;
    if (remaining <= 0 || out.length >= MAX_FOUND) break;
    const limit = Math.min(to, from + remaining);
    bytes.setRange(from, limit);
    activeRangeFrom = from;
    rangeSearched = 0;
    nextYield = from + YIELD_EVERY;
    // The last open-ended match has no format-specific end marker.
    let open: Embedded | null = null;
    for (let i = from; i < limit && out.length < MAX_FOUND; i++) {
      if (options.signal?.aborted) throw new DOMException("Embedded file search cancelled", "AbortError");
      if (i >= nextYield) await checkpoint(i);
      rangeSearched = Math.max(rangeSearched, i - from + 1);
      if (!covered(bytes, i - 4, i + 16, from, limit)) {
        await bytes.load(Math.max(from, i - 4), Math.min(limit, i + 16));
      }
      const candidates = BY_FIRST.get(bytes.at(i));
      if (!candidates) continue;
      for (const s of candidates) {
        if (skipped.has(s.ext)) continue;
        const start = i - (s.at ?? 0);
        if (start < from || start + (options.baseOffset ?? 0) <= 0 || (open && start <= open.start)) continue;
        let k = 0;
        while (k < s.magic.length && i + k < limit && bytes.at(i + k) === s.magic[k]) k++;
        if (k < s.magic.length || start + (s.headerBytes ?? 0) > limit || (s.ok && !s.ok(bytes, start))) continue;
        if (open) open.end = start;
        const end = s.end ? await s.end(bytes, start, limit, checkpoint) : -1;
        const found: Embedded = { start, end: end > start ? end : limit, what: s.what, ext: s.ext };
        out.push(found);
        if (end > start) {
          open = null;
          i = end - 1;
          rangeSearched = Math.max(rangeSearched, Math.min(limit, end) - from);
        } else {
          open = found;
        }
        break;
      }
    }
    if (out.length < MAX_FOUND) rangeSearched = limit - from;
    searched += rangeSearched;
    options.onProgress?.({ searched: Math.min(total, searched), total, matches: out.length });
    if (limit < to) break;
  }

  const matchLimitReached = out.length >= MAX_FOUND;
  return { matches: out, searched, available, limited: searched < available || matchLimitReached, matchLimitReached };
}

/** Inspect available original-offset views independently, never joining across a gap. */
export async function findEmbeddedRanges(
  ranges: readonly [number, number][],
  view: (start: number, end: number) => Uint8Array | undefined,
  options: EmbeddedSearchOptions = {},
): Promise<EmbeddedSearchResult> {
  const available = ranges.reduce((n, [a, b]) => n + Math.max(0, b - a), 0);
  const total = Math.min(available, Math.max(0, Math.min(MAX_SEARCHED, options.maxBytes ?? MAX_SEARCHED)));
  const matches: Embedded[] = [];
  let searched = 0;
  for (const [start, end] of ranges) {
    if (options.signal?.aborted) throw new DOMException("Embedded file search cancelled", "AbortError");
    if (searched >= total || matches.length >= MAX_FOUND) break;
    const bytes = view(start, Math.min(end, start + total - searched));
    if (!bytes?.length) continue;
    const result = await findEmbedded(bytes, [[0, bytes.length]], [], {
      ...options,
      baseOffset: start,
      maxBytes: total - searched,
      onProgress: (p) => options.onProgress?.({ searched: searched + p.searched, total, matches: Math.min(MAX_FOUND, matches.length + p.matches) }),
    });
    for (const match of result.matches) {
      if (matches.length >= MAX_FOUND) break;
      matches.push({ ...match, start: start + match.start, end: start + match.end });
    }
    searched += result.searched;
    if (result.matchLimitReached) break;
  }
  const matchLimitReached = matches.length >= MAX_FOUND;
  return { matches, searched, available, limited: searched < available || matchLimitReached, matchLimitReached };
}
