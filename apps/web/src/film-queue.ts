/** One active raster and one newest pending request, across every lab view. */
export class LatestFilmQueue<T> {
  private active = false;
  private pending: T | undefined;
  constructor(private readonly run: (request: T) => Promise<void>, private readonly discard: (request: T) => void) {}
  push(request: T): void {
    if (this.active) {
      if (this.pending !== undefined) this.discard(this.pending);
      this.pending = request;
      return;
    }
    this.active = true;
    void Promise.resolve().then(() => this.run(request)).finally(() => {
      this.active = false;
      const next = this.pending; this.pending = undefined;
      if (next !== undefined) this.push(next);
    }).catch(() => { /* The worker's run callback reports failures to its caller. */ });
  }
}
