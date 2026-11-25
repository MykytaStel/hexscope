/** Fixed-budget cache for deferred media. A page is published only after a
 * complete read. In-flight reads are deduplicated and capped.
 */
export const BYTE_PAGE_SIZE = 64 * 1024;
export const BYTE_CACHE_PAGES = 64;
const MAX_PENDING = 4;

export class BytePageCache {
  private readonly pages = new Map<number, Uint8Array>();
  private readonly pending = new Map<number, Promise<boolean>>();

  constructor(private readonly source: Blob) {}

  get retainedBytes(): number {
    let bytes = 0;
    for (const page of this.pages.values()) bytes += page.length;
    return bytes;
  }

  byteAt(offset: number): number | undefined {
    if (offset < 0 || offset >= this.source.size) return undefined;
    const index = Math.floor(offset / BYTE_PAGE_SIZE);
    const page = this.pages.get(index);
    if (!page) return undefined;
    // Refresh recency on access.
    this.pages.delete(index);
    this.pages.set(index, page);
    return page[offset - index * BYTE_PAGE_SIZE];
  }

  private read(index: number): Promise<boolean> {
    if (this.pages.has(index)) return Promise.resolve(false);
    const pending = this.pending.get(index);
    if (pending) return pending;
    if (this.pending.size >= MAX_PENDING) return Promise.resolve(false);
    const start = index * BYTE_PAGE_SIZE;
    const end = Math.min(this.source.size, start + BYTE_PAGE_SIZE);
    const work = Promise.resolve().then(async () => {
      const page = new Uint8Array(await this.source.slice(start, end).arrayBuffer());
      if (page.length !== end - start) return false;
      this.pages.set(index, page);
      while (this.pages.size > BYTE_CACHE_PAGES) {
        const oldest = this.pages.keys().next().value;
        if (oldest === undefined) break;
        this.pages.delete(oldest);
      }
      return true;
    }).catch(() => false).finally(() => this.pending.delete(index));
    this.pending.set(index, work);
    return work;
  }

  /** A visible range is at most four pages per request, including misalignment. */
  async load(start: number, end: number): Promise<boolean> {
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || start >= end || end > this.source.size) return false;
    const first = Math.floor(start / BYTE_PAGE_SIZE);
    const last = Math.min(first + MAX_PENDING - 1, Math.floor((end - 1) / BYTE_PAGE_SIZE));
    const reads: Promise<boolean>[] = [];
    for (let index = first; index <= last; index++) reads.push(this.read(index));
    return (await Promise.all(reads)).some(Boolean);
  }
}
