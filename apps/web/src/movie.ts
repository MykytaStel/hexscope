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
const RANGE_CHUNK = 1024 * 1024;
const ENTROPY_WORKERS = 8;

export type MovieProgressStage = "scan" | "ranges" | "entropy" | "parse";

export interface MovieProgress {
  stage: MovieProgressStage;
  loaded: number;
  total: number;
  cancellable: boolean;
}

export interface MovieReadOptions {
  signal?: AbortSignal;
  onProgress?: (progress: MovieProgress) => void;
}

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

/** Reads only the first box needed to identify a movie, never bytes from its following media box. */
async function movieHead(file: Blob): Promise<Uint8Array> {
  const first = await bytesOf(file, 0, Math.min(file.size, 8));
  if (String.fromCharCode(...first.subarray(4, 8)) !== "ftyp") return first;
  const size32 = u32(first, 0);
  const header = size32 === 1 ? 16 : 8;
  let size = size32;
  if (size32 === 1) {
    const extended = await bytesOf(file, 8, Math.min(file.size, 16));
    if (extended.length < 8) return first;
    size = u32(extended, 0) * 2 ** 32 + u32(extended, 4);
  } else if (size32 === 0) {
    size = file.size;
  }
  if (size < header || size > file.size) return first;
  return size <= HEAD ? bytesOf(file, 0, size) : bytesOf(file, 0, HEAD);
}

function checkCancelled(signal?: AbortSignal): void {
  if (signal?.aborted) throw new Error("Reading cancelled");
}

function report(options: MovieReadOptions, stage: MovieProgressStage, loaded: number, total: number): void {
  options.onProgress?.({ stage, loaded, total, cancellable: stage !== "parse" });
}

/** The boxes at the top of the file, up to the first that does not fit. */
async function topBox(file: Blob, at: number, signal?: AbortSignal): Promise<Box | null> {
  checkCancelled(signal);
  // Read only the ordinary eight-byte header. Reading sixteen bytes every
  // time also reads the first eight bytes of a following mdat body.
  const headerBytes = await bytesOf(file, at, at + 8);
  checkCancelled(signal);
  if (headerBytes.length < 8) return null;
  const size32 = u32(headerBytes, 0);
  const header = size32 === 1 ? 16 : 8;
  let size = size32;
  if (size32 === 1) {
    const extended = await bytesOf(file, at + 8, at + 16);
    checkCancelled(signal);
    if (extended.length < 8) return null;
    size = u32(extended, 0) * 2 ** 32 + u32(extended, 4);
  } else if (size32 === 0) {
    size = file.size - at;
  }
  if (size < header || at + size > file.size) return null;
  return { at, header, size, type: String.fromCharCode(...headerBytes.subarray(4, 8)) };
}

async function topBoxes(file: Blob, options: MovieReadOptions): Promise<Box[]> {
  const boxes: Box[] = [];
  let at = 0;
  let reportedAt = 0;
  while (at + 8 <= file.size && boxes.length < MAX_BOXES) {
    const box = await topBox(file, at, options.signal);
    if (!box) break;
    boxes.push(box);
    at += box.size;
    if (at - reportedAt >= RANGE_CHUNK) {
      report(options, "scan", at, file.size);
      reportedAt = at;
    }
  }
  checkCancelled(options.signal);
  report(options, "scan", at, file.size);
  return boxes;
}

/** The movie in parts, or null when `file` is not a large movie with media worth leaving out. */
export async function readMovie(
  file: Blob,
  isMovie: (head: Uint8Array, len: number) => boolean,
  from = LARGE_MOVIE,
  options: MovieReadOptions = {},
): Promise<Movie | null> {
  checkCancelled(options.signal);
  if (file.size < from || !isMovie(await movieHead(file), file.size)) return null;
  checkCancelled(options.signal);
  report(options, "scan", 0, file.size);
  const boxes = await topBoxes(file, options);
  checkCancelled(options.signal);
  const media = boxes.filter((b) => b.type === "mdat" && b.size - b.header >= MIN_GAP);
  if (media.length === 0) return null;

  const omitted = media.reduce((n, box) => n + box.size - box.header, 0);
  const given = new Uint8Array(file.size - omitted);
  const whole = new Uint8Array(file.size);
  const gaps: number[] = [];
  const joins: Movie["joins"] = [];
  const missing: number[] = [];
  let at = 0;
  let givenAt = 0;
  report(options, "ranges", 0, given.length);
  const take = async (end: number) => {
    while (at < end) {
      checkCancelled(options.signal);
      const next = Math.min(end, at + RANGE_CHUNK);
      const b = await bytesOf(file, at, next);
      checkCancelled(options.signal);
      whole.set(b, at);
      given.set(b, givenAt);
      at += b.length;
      givenAt += b.length;
      report(options, "ranges", givenAt, given.length);
    }
  };
  for (const box of media) {
    const body = box.at + box.header;
    const len = box.size - box.header;
    await take(body);
    gaps.push(givenAt - box.header, len);
    joins.push({ at: givenAt, start: body, len });
    missing.push(body, len);
    at = box.at + box.size;
  }
  await take(file.size);
  checkCancelled(options.signal);

  return { given, gaps: Float64Array.from(gaps), joins, whole, missing: Float64Array.from(missing) };
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
export async function movieEntropy(
  file: Blob,
  movie: Movie,
  bins: number,
  entropy: (b: Uint8Array) => number,
  options: MovieReadOptions = {},
): Promise<{ values: Float32Array; window: number }> {
  checkCancelled(options.signal);
  const n = Math.max(1, Math.min(bins, Math.floor(file.size / 256)));
  const window = Math.ceil(file.size / n);
  const missing: [number, number][] = [];
  for (let i = 0; i < movie.missing.length; i += 2) missing.push([movie.missing[i], movie.missing[i] + movie.missing[i + 1]]);
  const samples: { start: number; end: number; mediaAt: number | null }[] = [];
  let mediaIndex = 0;
  for (let i = 0; i < n; i++) {
    const start = i * window;
    const end = Math.min(file.size, start + window);
    while (mediaIndex < missing.length && missing[mediaIndex][1] <= start) {
      mediaIndex++;
    }
    const next = missing[mediaIndex];
    const mediaAt = next && start < next[1] && end > next[0] ? Math.max(start, next[0]) : null;
    samples.push({ start: mediaAt === null ? start : mediaAt, end: mediaAt === null ? end : Math.min(end, mediaAt + 4096), mediaAt });
  }
  const values = new Float32Array(n);
  let next = 0;
  let done = 0;
  report(options, "entropy", 0, n);
  const sample = async () => {
    while (true) {
      checkCancelled(options.signal);
      const i = next++;
      if (i >= n) return;
      const { start, end, mediaAt } = samples[i];
      const bytes = mediaAt === null ? movie.whole.subarray(start, end) : await bytesOf(file, start, end);
      checkCancelled(options.signal);
      values[i] = entropy(bytes);
      done++;
      if (done % 16 === 0 || done === n) report(options, "entropy", done, n);
    }
  };
  await Promise.all(Array.from({ length: Math.min(ENTROPY_WORKERS, n) }, sample));
  return { values: Float32Array.from(values), window };
}
