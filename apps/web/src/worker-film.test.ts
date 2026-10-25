import { describe, expect, it } from "vitest";
import { createFilmQueue, type FilmOperationRequest } from "./worker-film";

function deferred<T = void>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

const request = (id: number) => ({ id } as unknown as FilmOperationRequest);

describe("film worker queue", () => {
  it("runs only the latest pending request and reports the displaced request once", async () => {
    const first = deferred();
    const latestStarted = deferred();
    const started: number[] = [];
    const discarded: number[] = [];
    const queue = createFilmQueue(async ({ id }) => {
      started.push(id);
      if (id === 1) await first.promise;
      if (id === 3) latestStarted.resolve();
    }, ({ id }) => discarded.push(id));

    queue.push(request(1));
    queue.push(request(2));
    queue.push(request(3));
    await Promise.resolve();

    expect(started).toEqual([1]);
    expect(discarded).toEqual([2]);

    first.resolve();
    await latestStarted.promise;
    expect(started).toEqual([1, 3]);
    expect(discarded).toEqual([2]);
  });

  it("continues with the latest pending request after a run rejects", async () => {
    const latestStarted = deferred();
    const started: number[] = [];
    const queue = createFilmQueue(async ({ id }) => {
      started.push(id);
      if (id === 1) throw new Error("render failed");
      latestStarted.resolve();
    }, () => {});

    queue.push(request(1));
    queue.push(request(2));
    await latestStarted.promise;

    expect(started).toEqual([1, 2]);
  });
});
