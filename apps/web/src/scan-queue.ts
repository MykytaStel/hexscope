/** One active image-analysis job per reader, including film export readback.
 * Queued cancellations release their closures immediately; running jobs
 * hold the lane until their cooperative cleanup has actually finished.
 */
export class SerialScanQueue {
  private active = false;
  private readonly waiting: (() => void)[] = [];

  run<T>(signal: AbortSignal, task: () => Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      if (signal.aborted) {
        reject(signal.reason);
        return;
      }
      const abort = () => {
        const index = this.waiting.indexOf(start);
        if (index < 0) return;
        this.waiting.splice(index, 1);
        signal.removeEventListener("abort", abort);
        reject(signal.reason);
      };
      const start = () => {
        signal.removeEventListener("abort", abort);
        this.active = true;
        void Promise.resolve().then(() => {
          signal.throwIfAborted();
          return task();
        }).then(resolve, reject).finally(() => {
          this.active = false;
          this.waiting.shift()?.();
        });
      };
      if (this.active) {
        this.waiting.push(start);
        signal.addEventListener("abort", abort, { once: true });
      } else {
        start();
      }
    });
  }
}
