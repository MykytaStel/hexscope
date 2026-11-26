import { describe, expect, it, vi } from "vitest";
import { SerialScanQueue } from "./scan-queue";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

describe("serial image analysis", () => {
  it("keeps the active lane until actual cleanup and skips cancelled queued work", async () => {
    const queue = new SerialScanQueue();
    const active = new AbortController();
    const queued = new AbortController();
    const gate = deferred<number>();
    const first = vi.fn(() => gate.promise);
    const skipped = vi.fn(async () => 2);
    const last = vi.fn(async () => 3);
    const a = queue.run(active.signal, first);
    const b = queue.run(queued.signal, skipped);
    const c = queue.run(new AbortController().signal, last);
    await Promise.resolve();
    expect(first).toHaveBeenCalledOnce();
    expect(last).not.toHaveBeenCalled();
    queued.abort();
    await expect(b).rejects.toMatchObject({ name: "AbortError" });
    active.abort();
    expect(last).not.toHaveBeenCalled();
    gate.resolve(1);
    await expect(a).resolves.toBe(1);
    await expect(c).resolves.toBe(3);
    expect(skipped).not.toHaveBeenCalled();
    expect(last).toHaveBeenCalledOnce();
  });

  it("continues after failure and does not start already cancelled requests", async () => {
    const queue = new SerialScanQueue();
    const signal = new AbortController().signal;
    const rejected = queue.run(signal, async () => { throw new Error("bad image"); });
    const next = queue.run(signal, async () => "next");
    await expect(rejected).rejects.toThrow("bad image");
    await expect(next).resolves.toBe("next");
    const aborted = new AbortController();
    aborted.abort();
    const task = vi.fn(async () => 1);
    await expect(queue.run(aborted.signal, task)).rejects.toMatchObject({ name: "AbortError" });
    expect(task).not.toHaveBeenCalled();
  });
});
