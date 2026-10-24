import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

interface PostedRequest {
  id: number;
  type: string;
  [key: string]: unknown;
}

class FakeWorker {
  static instances: FakeWorker[] = [];
  readonly sent: PostedRequest[] = [];
  terminated = false;
  private readonly listeners = new Map<string, EventListenerOrEventListenerObject[]>();

  constructor() {
    FakeWorker.instances.push(this);
  }

  addEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  postMessage(message: PostedRequest): void {
    this.sent.push(message);
  }

  terminate(): void {
    this.terminated = true;
  }

  respond(response: Record<string, unknown>): void {
    const event = { data: response } as MessageEvent;
    for (const listener of this.listeners.get("message") ?? []) {
      if (typeof listener === "function") listener.call(this, event);
      else listener.handleEvent(event);
    }
  }
}

describe("worker RPC timeouts", () => {
  beforeEach(() => {
    FakeWorker.instances = [];
    vi.resetModules();
    vi.stubGlobal("Worker", FakeWorker);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("terminates a busy reader after verification times out and starts a fresh worker", async () => {
    const rpc = await import("./rpc");
    const worker = FakeWorker.instances[0];
    const pending = rpc.call({ type: "steps", from: 0, count: 1 });
    const pendingId = worker.sent[0].id;
    const timed = rpc.call(
      {
        type: "verifyCopy",
        copy: new Blob(["copy"]),
        sourceFormat: "jpeg",
        sourceFindings: [],
        retainedReasons: {},
      },
      25,
    );
    const timedId = worker.sent[1].id;
    let settlements = 0;
    void timed.then(() => settlements++);

    await vi.advanceTimersByTimeAsync(25);
    await Promise.resolve();
    expect(settlements).toBe(1);
    expect(worker.terminated).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
    await expect(timed).resolves.toMatchObject({
      id: timedId,
      type: "error",
      message: "copy verification did not finish in time",
    });
    await expect(pending).resolves.toMatchObject({
      id: pendingId,
      type: "error",
      message: expect.stringContaining("reopen the file"),
    });

    let laterSettled = false;
    const later = rpc.call({ type: "pageTexts", bytes: new Uint8Array() }).then((response) => {
      laterSettled = true;
      return response;
    });
    const replacement = FakeWorker.instances[1];
    const laterId = replacement.sent[0].id;
    worker.respond({ id: timedId, type: "verification", report: { removed: [], present: [], unchecked: [] } });
    await Promise.resolve();
    expect(laterSettled).toBe(false);
    replacement.respond({ id: laterId, type: "pageTexts", pages: [] });
    await expect(later).resolves.toMatchObject({ id: laterId, type: "pageTexts", pages: [] });
  });
});
