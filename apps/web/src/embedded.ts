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
}

/** Files listed, at most. */
const MAX_FOUND = 100;
/** Bytes searched, at most: past this, the rest is not looked at. */
const MAX_SEARCHED = 256 * 1024 * 1024;
/** Bound uninterrupted work so a large file cannot monopolize the UI thread. */
const YIELD_EVERY = 256 * 1024;

const ascii = (s: string) => Uint8Array.from(s, (c) => c.charCodeAt(0));
type Checkpoint = (offset: number) => Promise<void>;

interface Signature {
  magic: Uint8Array;
  /** Where the magic sits from the file's start. */
  at?: number;
  what: string;
  ext: string;
  /** More than the magic that must hold, so noise is not taken for a file. */
  ok?: (b: Uint8Array, i: number) => boolean;
  /** Where the file ends, when its format says; otherwise it runs to the next one found. */
  end?: (b: Uint8Array, i: number, limit: number, checkpoint: Checkpoint) => Promise<number>;
}

const be32 = (b: Uint8Array, i: number) => ((b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]) >>> 0;
const le16 = (b: Uint8Array, i: number) => b[i] | (b[i + 1] << 8);

async function indexOf(b: Uint8Array, needle: Uint8Array, from: number, limit: number, checkpoint: Checkpoint): Promise<number> {
  let nextYield = from + YIELD_EVERY;
  outer: for (let i = from; i + needle.length <= limit; i++) {
    if (i >= nextYield) {
      await checkpoint(i);
      nextYield = i + YIELD_EVERY;
    }
    for (let k = 0; k < needle.length; k++) if (b[i + k] !== needle[k]) continue outer;
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
    ok: (b, i) => (b[i + 3] >= 0xe0 && b[i + 3] <= 0xef) || b[i + 3] === 0xdb || b[i + 3] === 0xc0 || b[i + 3] === 0xc4 || b[i + 3] === 0xfe,
    // Segment by segment to the first scan — a thumbnail inside is skipped
    // whole — then to the end marker, which scan data never holds.
    end: async (b, i, limit, checkpoint) => {
      let k = i + 2;
      let checkpointAt = i + YIELD_EVERY;
      while (k + 4 <= limit && b[k] === 0xff) {
        if (k >= checkpointAt) {
          await checkpoint(k);
          checkpointAt = k + YIELD_EVERY;
        }
        const marker = b[k + 1];
        if (marker === 0xda) break;
        if (marker === 0xd8 || (marker >= 0xd0 && marker <= 0xd7) || marker === 0x01 || marker === 0xff) {
          k += marker === 0xff ? 1 : 2;
          continue;
        }
        k += 2 + ((b[k + 2] << 8) | b[k + 3]);
      }
      for (; k + 1 < limit; k++) {
        if (k >= checkpointAt) {
          await checkpoint(k);
          checkpointAt = k + YIELD_EVERY;
        }
        if (b[k] === 0xff && b[k + 1] === 0xd9) return k + 2;
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
    ok: (b, i) => b[i + 8] === 0x57 && b[i + 9] === 0x45 && b[i + 10] === 0x42 && b[i + 11] === 0x50,
    end: async (b, i, limit) => Math.min(limit, i + 8 + (b[i + 4] | (b[i + 5] << 8) | (b[i + 6] << 16) | (b[i + 7] << 24))),
  },
  {
    magic: Uint8Array.of(0x50, 0x4b, 0x03, 0x04),
    what: "a ZIP archive",
    ext: "zip",
    end: async (b, i, limit, checkpoint) => {
      const at = await indexOf(b, EOCD, i + 4, limit, checkpoint);
      return at < 0 || at + 22 > limit ? -1 : Math.min(limit, at + 22 + le16(b, at + 20));
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
    ok: (b, i) => {
      const size = be32(b, i);
      return size >= 16 && size <= 256 && /^[\x20-\x7e]{4}$/.test(String.fromCharCode(b[i + 8], b[i + 9], b[i + 10], b[i + 11]));
    },
  },
  { magic: Uint8Array.of(0x1f, 0x8b, 0x08), what: "a gzip archive", ext: "gz" },
  { magic: Uint8Array.of(0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c), what: "a 7-Zip archive", ext: "7z" },
  { magic: ascii("Rar!\x1a\x07"), what: "a RAR archive", ext: "rar" },
  { magic: Uint8Array.of(0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00), what: "a WebAssembly module", ext: "wasm" },
  { magic: Uint8Array.of(0x7f, 0x45, 0x4c, 0x46), what: "an ELF program", ext: "elf", ok: (b, i) => b[i + 4] === 1 || b[i + 4] === 2 },
  { magic: ascii("OggS\x00"), what: "an Ogg audio or video file", ext: "ogg" },
  { magic: ascii("ID3"), what: "an MP3 file", ext: "mp3", ok: (b, i) => b[i + 3] >= 2 && b[i + 3] <= 4 && b[i + 5] < 0x80 },
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
export async function findEmbedded(
  bytes: Uint8Array,
  ranges: readonly [number, number][],
  skip: readonly string[] = [],
  options: EmbeddedSearchOptions = {},
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
    activeRangeFrom = from;
    rangeSearched = 0;
    nextYield = from + YIELD_EVERY;
    // The last open-ended match has no format-specific end marker.
    let open: Embedded | null = null;
    for (let i = from; i < limit && out.length < MAX_FOUND; i++) {
      if (options.signal?.aborted) throw new DOMException("Embedded file search cancelled", "AbortError");
      if (i >= nextYield) await checkpoint(i);
      rangeSearched = Math.max(rangeSearched, i - from + 1);
      const candidates = BY_FIRST.get(bytes[i]);
      if (!candidates) continue;
      for (const s of candidates) {
        if (skipped.has(s.ext)) continue;
        const start = i - (s.at ?? 0);
        if (start <= 0 || start < from || (open && start <= open.start)) continue;
        let k = 0;
        while (k < s.magic.length && i + k < limit && bytes[i + k] === s.magic[k]) k++;
        if (k < s.magic.length || (s.ok && !s.ok(bytes, start))) continue;
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
