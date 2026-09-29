// A movie too large to read whole. Its boxes are listed by their headers,
// and all of it is read except the picture and sound: the parser gets the
// rest, and the page gets the file's length of bytes with the media left
// to read when it is looked at. A clean copy is its changed bytes with the
// media taken from the file as it is.

/** Movies past this size are read in parts. */
export const LARGE_MOVIE = 64 * 1024 * 1024;
/** Media shorter than this is read with the rest. */
const MIN_GAP = 1024 * 1024;
const MAX_BOXES = 100_000;
const HEAD = 64 * 1024;

export interface Movie {
  /** The file without its media: what the parser reads. */
  given: Uint8Array;
  /** Per media box, where its header starts in `given` and its body's length. */
  gaps: Float64Array;
  /** Per media box, where its body goes back: after its header in `given`, from `start` in the file. */
  joins: { at: number; start: number; len: number }[];
  /** The file's length of bytes, the media not read. */
  whole: Uint8Array;
  /** The media in the file, as `[start, len, …]`. */
  missing: Float64Array;
}

interface Box {
  at: number;
  header: number;
  size: number;
  type: string;
}

async function bytesOf(file: Blob, start: number, end: number): Promise<Uint8Array> {
  return new Uint8Array(await file.slice(start, end).arrayBuffer());
}

const u32 = (b: Uint8Array, at: number) => ((b[at] << 24) | (b[at + 1] << 16) | (b[at + 2] << 8) | b[at + 3]) >>> 0;

/** The boxes at the top of the file, up to the first that does not fit. */
async function topBoxes(file: Blob): Promise<Box[]> {
  const boxes: Box[] = [];
  let at = 0;
  while (at + 8 <= file.size && boxes.length < MAX_BOXES) {
    const h = await bytesOf(file, at, at + 16);
    const size32 = u32(h, 0);
    const header = size32 === 1 ? 16 : 8;
    const size = size32 === 1 ? (h.length < 16 ? 0 : u32(h, 8) * 2 ** 32 + u32(h, 12)) : size32 === 0 ? file.size - at : size32;
    if (size < header || at + size > file.size) break;
    boxes.push({ at, header, size, type: String.fromCharCode(...h.subarray(4, 8)) });
    at += size;
  }
  return boxes;
}

/** The movie in parts, or null when `file` is not a large movie with media worth leaving out. */
export async function readMovie(file: Blob, isMovie: (head: Uint8Array, len: number) => boolean, from = LARGE_MOVIE): Promise<Movie | null> {
  if (file.size < from || !isMovie(await bytesOf(file, 0, HEAD), file.size)) return null;
  const media = (await topBoxes(file)).filter((b) => b.type === "mdat" && b.size - b.header >= MIN_GAP);
  if (media.length === 0) return null;

  const whole = new Uint8Array(file.size);
  const parts: Uint8Array[] = [];
  const gaps: number[] = [];
  const joins: Movie["joins"] = [];
  const missing: number[] = [];
  let at = 0;
  let given = 0;
  const take = async (end: number) => {
    const b = await bytesOf(file, at, end);
    whole.set(b, at);
    parts.push(b);
    given += b.length;
  };
  for (const box of media) {
    const body = box.at + box.header;
    const len = box.size - box.header;
    await take(body);
    gaps.push(given - box.header, len);
    joins.push({ at: given, start: body, len });
    missing.push(body, len);
    at = box.at + box.size;
  }
  await take(file.size);

  const bytes = new Uint8Array(given);
  let o = 0;
  for (const p of parts) {
    bytes.set(p, o);
    o += p.length;
  }
  return { given: bytes, gaps: Float64Array.from(gaps), joins, whole, missing: Float64Array.from(missing) };
}

/** The clean copy of a movie: `copy`, the parser's copy of `given`, with the media put back from the file. */
export function assemble(copy: Uint8Array, movie: Movie, file: Blob): Blob {
  const parts: BlobPart[] = [];
  let from = 0;
  for (const j of movie.joins) {
    parts.push(copy.subarray(from, j.at) as BlobPart, file.slice(j.start, j.start + j.len));
    from = j.at;
  }
  parts.push(copy.subarray(from) as BlobPart);
  return new Blob(parts);
}

/**
 * Entropy for the minimap, as for any file, except that each window in the
 * media is judged by a sample of it read from the file.
 */
export async function movieEntropy(file: Blob, movie: Movie, bins: number, entropy: (b: Uint8Array) => number): Promise<{ values: Float32Array; window: number }> {
  const n = Math.max(1, Math.min(bins, Math.floor(file.size / 256)));
  const window = Math.ceil(file.size / n);
  const missing: [number, number][] = [];
  for (let i = 0; i < movie.missing.length; i += 2) missing.push([movie.missing[i], movie.missing[i] + movie.missing[i + 1]]);
  const values = await Promise.all(
    Array.from({ length: n }, async (_, i) => {
      const start = i * window;
      const end = Math.min(file.size, start + window);
      const inMedia = missing.some(([a, b]) => start < b && end > a);
      return entropy(inMedia ? await bytesOf(file, start, Math.min(end, start + 4096)) : movie.whole.subarray(start, end));
    }),
  );
  return { values: Float32Array.from(values), window };
}
