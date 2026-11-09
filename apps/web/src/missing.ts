// A large movie's picture and sound are not read with the rest of it: the
// page has no bytes there until the bytes view reaches them, and then reads
// them from the file a chunk at a time.

const CHUNK = 64 * 1024;

export class Missing {
  /** `[start, end)`, in file order. */
  readonly ranges: [number, number][] = [];
  private readonly loaded = new Set<number>();
  private reading = false;

  /** From `[start, len, …]`. */
  constructor(flat: Float64Array) {
    for (let i = 0; i + 1 < flat.length; i += 2) this.ranges.push([flat[i], flat[i] + flat[i + 1]]);
  }

  /** Bytes the page was not given. */
  get total(): number {
    return this.ranges.reduce((n, [a, b]) => n + b - a, 0);
  }

  private inRange(offset: number): boolean {
    const index = this.firstEndingAfter(offset);
    const range = this.ranges[index];
    return range !== undefined && offset >= range[0];
  }

  /** First range whose exclusive end is greater than `offset`. */
  private firstEndingAfter(offset: number): number {
    let low = 0;
    let high = this.ranges.length;
    while (low < high) {
      const middle = low + Math.floor((high - low) / 2);
      if (this.ranges[middle][1] <= offset) low = middle + 1;
      else high = middle;
    }
    return low;
  }

  /** Whether the byte at `offset` is here. */
  has(offset: number): boolean {
    return !this.inRange(offset) || this.loaded.has(Math.floor(offset / CHUNK));
  }

  /** What to read for `[start, end)` to be here: whole chunks, within the missing ranges. */
  wanted(start: number, end: number): [number, number][] {
    const out: [number, number][] = [];
    for (let c = Math.floor(start / CHUNK); c * CHUNK < end; c++) {
      if (this.loaded.has(c)) continue;
      const chunkStart = c * CHUNK;
      const chunkEnd = chunkStart + CHUNK;
      for (let i = this.firstEndingAfter(chunkStart); i < this.ranges.length; i++) {
        const [a, b] = this.ranges[i];
        if (a >= chunkEnd) break;
        const from = Math.max(a, c * CHUNK);
        const to = Math.min(b, chunkEnd);
        if (from < to) out.push([from, to]);
      }
    }
    return out;
  }

  /** Marks the chunks read by [`wanted`]'s ranges as here. */
  filled(ranges: [number, number][]): void {
    for (const [a] of ranges) this.loaded.add(Math.floor(a / CHUNK));
  }

  /**
   * Reads what `[start, end)` lacks from `source` into `bytes`. False when
   * there was nothing to read, a read is already under way, or the file
   * could not be read — moved or changed on disk.
   */
  async fill(bytes: Uint8Array, source: Blob, start: number, end: number): Promise<boolean> {
    const wanted = this.wanted(start, end);
    if (wanted.length === 0 || this.reading) return false;
    this.reading = true;
    try {
      for (const [a, b] of wanted) bytes.set(new Uint8Array(await source.slice(a, b).arrayBuffer()), a);
      this.filled(wanted);
      return true;
    } catch {
      return false;
    } finally {
      this.reading = false;
    }
  }

  /** The parts of `[0, length)` supplied during the initial parse. */
  read(length: number): [number, number][] {
    const out: [number, number][] = [];
    let at = 0;
    for (const [a, b] of this.ranges) {
      if (a > at) out.push([at, a]);
      at = b;
    }
    if (at < length) out.push([at, length]);
    return out;
  }

  /** The parts of `[0, length)` available now, including deferred chunks read since. */
  available(length: number): [number, number][] {
    const out: [number, number][] = [];
    const add = (start: number, end: number) => {
      const from = Math.max(0, Math.min(length, start));
      const to = Math.max(from, Math.min(length, end));
      if (from >= to) return;
      const previous = out[out.length - 1];
      if (previous && previous[1] >= from) previous[1] = Math.max(previous[1], to);
      else out.push([from, to]);
    };
    let at = 0;
    for (const [a, b] of this.ranges) {
      const start = Math.max(0, Math.min(length, a));
      const end = Math.max(start, Math.min(length, b));
      add(at, start);
      for (let chunk = Math.floor(start / CHUNK); chunk * CHUNK < end; chunk++) {
        if (this.loaded.has(chunk)) add(Math.max(start, chunk * CHUNK), Math.min(end, (chunk + 1) * CHUNK));
      }
      at = Math.max(at, end);
    }
    add(at, length);
    return out;
  }
}
