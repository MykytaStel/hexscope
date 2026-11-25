/** A compact byte buffer with omitted ranges mapped to original offsets.
 * A view is zero-copy; missing data is never represented as a real zero byte.
 */
export class CompactByteSource {
  constructor(
    readonly size: number,
    private readonly bytes: Uint8Array,
    private readonly gaps: readonly { at: number; start: number; len: number }[],
  ) {}

  private offset(start: number, end: number): number | undefined {
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end < start || end > this.size) return undefined;
    let low = 0;
    let high = this.gaps.length;
    while (low < high) {
      const mid = low + Math.floor((high - low) / 2);
      const gap = this.gaps[mid];
      if (gap.start + gap.len <= start) low = mid + 1;
      else high = mid;
    }
    const gap = this.gaps[low];
    if (gap && start < gap.start + gap.len && end > gap.start) return undefined;
    const prev = this.gaps[low - 1];
    const omitted = prev ? prev.start + prev.len - prev.at : 0;
    const at = start - omitted;
    return at >= 0 && at + end - start <= this.bytes.length ? at : undefined;
  }

  hasRange(start: number, end: number): boolean {
    return this.offset(start, end) !== undefined;
  }

  view(start: number, end: number): Uint8Array | undefined {
    const at = this.offset(start, end);
    return at === undefined ? undefined : this.bytes.subarray(at, at + end - start);
  }
}
