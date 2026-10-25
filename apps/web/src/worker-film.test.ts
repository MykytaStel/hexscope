import { describe, expect, it, vi } from "vitest";
import { renderFilm } from "./film-render";
import { createFilmQueue, handleFilmOperation, type FilmOperationRequest, type FilmWorkerServices } from "./worker-film";
import type { FilmRenderResult } from "./film-render";
import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";

vi.mock("./film-render", () => ({ renderFilm: vi.fn() }));

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

describe("film worker operations", () => {
  it("rejects an oversized film scan before reading or loading a parser", async () => {
    const slice = vi.fn();
    const source = { size: 50 * 1024 * 1024 + 1, slice } as unknown as Blob;
    const services = {
      moduleFor: vi.fn(),
      loadMedia: vi.fn(),
      describe: vi.fn(),
      codes: vi.fn(),
      post: vi.fn(),
    } as unknown as FilmWorkerServices;
    const request = {
      id: 8,
      type: "filmRoll",
      source,
      settings: {},
      format: "jpeg",
    } as unknown as FilmOperationRequest;

    await expect(handleFilmOperation(request, services)).rejects.toThrow("This scan exceeds the 50 MiB file limit.");
    expect(slice).not.toHaveBeenCalled();
    expect(services.moduleFor).not.toHaveBeenCalled();
    expect(services.loadMedia).not.toHaveBeenCalled();
  });

  it("keeps an exported copy when readback fails and leaves its check unavailable", async () => {
    vi.mocked(renderFilm).mockReset();
    const copy = new Blob(["encoded image"]);
    const result = { copy, dimensions: [2, 2] } as unknown as FilmRenderResult;
    vi.mocked(renderFilm).mockResolvedValue(result);

    const parsed = { free: vi.fn() } as unknown as Parsed;
    const wasm = { parseOwned: vi.fn(() => parsed) };
    const post = vi.fn();
    const services = {
      moduleFor: vi.fn(),
      loadMedia: vi.fn().mockResolvedValue(wasm),
      describe: vi.fn(() => { throw new Error("copy cannot be completely read"); }),
      codes: vi.fn(),
      post,
    } as unknown as FilmWorkerServices;
    const request = {
      id: 9,
      type: "filmRender",
      source: new Blob(["source"]),
      dimensions: [2, 2],
      orientation: 1,
      settings: {},
      purpose: "export",
    } as unknown as FilmOperationRequest;

    await handleFilmOperation(request, services);

    expect(parsed.free).toHaveBeenCalledOnce();
    expect(post).toHaveBeenCalledWith(expect.objectContaining({
      id: 9,
      type: "filmRendered",
      result: expect.objectContaining({
        copy,
        check: { status: "unavailable", metadata: [], qrCount: null },
      }),
    }));
  });
});
