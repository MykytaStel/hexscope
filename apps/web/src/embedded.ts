// Files inside files: where another file's signature appears in the bytes,
// what it is, and how far it goes — a picture tacked onto the end of
// another, a ZIP behind a JPEG, the photos inside a PDF, the thumbnail in
// a camera's EXIF. Found by signature, the way binwalk finds them, and
// opened with the same parsers as any file.

export interface Embedded {
  start: number;
  end: number;
  /** "a JPEG picture" */
  what: string;
  /** The extension a saved copy gets. */
  ext: string;
}

/** Files listed, at most. */
const MAX_FOUND = 100;
/** Bytes searched, at most: past this, the rest is not looked at. */
const MAX_SEARCHED = 256 * 1024 * 1024;

const ascii = (s: string) => Uint8Array.from(s, (c) => c.charCodeAt(0));

interface Signature {
  magic: Uint8Array;
  /** Where the magic sits from the file's start. */
  at?: number;
  what: string;
  ext: string;
  /** More than the magic that must hold, so noise is not taken for a file. */
  ok?: (b: Uint8Array, i: number) => boolean;
  /** Where the file ends, when its format says; otherwise it runs to the next one found. */
  end?: (b: Uint8Array, i: number, limit: number) => number;
}

const be32 = (b: Uint8Array, i: number) => ((b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]) >>> 0;
const le16 = (b: Uint8Array, i: number) => b[i] | (b[i + 1] << 8);

function indexOf(b: Uint8Array, needle: Uint8Array, from: number, limit: number): number {
  outer: for (let i = from; i + needle.length <= limit; i++) {
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
    end: (b, i, limit) => {
      const at = indexOf(b, IEND, i + 8, limit);
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
    end: (b, i, limit) => {
      let k = i + 2;
      while (k + 4 <= limit && b[k] === 0xff) {
        const marker = b[k + 1];
        if (marker === 0xda) break;
        if (marker === 0xd8 || (marker >= 0xd0 && marker <= 0xd7) || marker === 0x01 || marker === 0xff) {
          k += marker === 0xff ? 1 : 2;
          continue;
        }
        k += 2 + ((b[k + 2] << 8) | b[k + 3]);
      }
      for (; k + 1 < limit; k++) if (b[k] === 0xff && b[k + 1] === 0xd9) return k + 2;
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
    end: (b, i, limit) => Math.min(limit, i + 8 + (b[i + 4] | (b[i + 5] << 8) | (b[i + 6] << 16) | (b[i + 7] << 24))),
  },
  {
    magic: Uint8Array.of(0x50, 0x4b, 0x03, 0x04),
    what: "a ZIP archive",
    ext: "zip",
    end: (b, i, limit) => {
      const at = indexOf(b, EOCD, i + 4, limit);
      return at < 0 || at + 22 > limit ? -1 : Math.min(limit, at + 22 + le16(b, at + 20));
    },
  },
  {
    magic: ascii("%PDF-"),
    what: "a PDF document",
    ext: "pdf",
    end: (b, i, limit) => {
      // The last end-of-file marker before the limit: updates add more.
      let last = -1;
      for (let at = indexOf(b, EOF, i, limit); at >= 0; at = indexOf(b, EOF, at + 5, limit)) last = at + 5;
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
 * Files whose signature starts within `[from, to)` of `bytes`, other than
 * one at the very start of the file (that is the file itself). Each runs to
 * where its format says it ends — and what lies inside it belongs to it —
 * or else to the next one found. Kinds in `skip` are not looked for: an
 * archive's own entries are not files hidden in it.
 */
export function findEmbedded(bytes: Uint8Array, from = 0, to = bytes.length, skip: string[] = []): Embedded[] {
  const limit = Math.min(to, from + MAX_SEARCHED);
  const out: Embedded[] = [];
  // The last one found whose format does not say where it ends.
  let open: Embedded | null = null;
  for (let i = from; i < limit && out.length < MAX_FOUND; i++) {
    const candidates = BY_FIRST.get(bytes[i]);
    if (!candidates) continue;
    for (const s of candidates) {
      if (skip.includes(s.ext)) continue;
      const start = i - (s.at ?? 0);
      if (start <= 0 || start < from || (open && start <= open.start)) continue;
      let k = 0;
      while (k < s.magic.length && bytes[i + k] === s.magic[k]) k++;
      if (k < s.magic.length || (s.ok && !s.ok(bytes, start))) continue;
      if (open) open.end = start;
      const end = s.end?.(bytes, start, to) ?? -1;
      const found: Embedded = { start, end: end > start ? end : to, what: s.what, ext: s.ext };
      out.push(found);
      if (end > start) {
        open = null;
        i = end - 1;
      } else {
        open = found;
      }
      break;
    }
  }
  return out;
}
