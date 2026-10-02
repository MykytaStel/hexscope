import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

interface PostedRequest {
  id: number;
  type: string;
  [key: string]: unknown;
}

class FakeWorker {
  static instances: FakeWorker[] = [];
  readonly sent: PostedRequest[] = [];
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

  terminate(): void {}

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

  it("resolves a verification timeout once, clears it, and leaves later requests alone", async () => {
    const rpc = await import("./rpc");
    const worker = FakeWorker.instances[0];
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
    const timedId = worker.sent[0].id;
    let settlements = 0;
    void timed.then(() => settlements++);

    await vi.advanceTimersByTimeAsync(25);
    await Promise.resolve();
    expect(settlements).toBe(1);
    expect(vi.getTimerCount()).toBe(0);

    // A late reply must not consume a later request's pending id.
    worker.respond({ id: timedId, type: "verification", report: { removed: [], present: [], unchecked: [] } });
    await Promise.resolve();
    expect(settlements).toBe(1);

    let laterSettled = false;
    const later = rpc.call({ type: "pageTexts", bytes: new Uint8Array() }).then((response) => {
      laterSettled = true;
      return response;
    });
    const laterId = worker.sent[1].id;
    await vi.advanceTimersByTimeAsync(100);
    await Promise.resolve();
    expect(laterSettled).toBe(false);
    worker.respond({ id: laterId, type: "pageTexts", pages: [] });
    await expect(later).resolves.toMatchObject({ id: laterId, type: "pageTexts", pages: [] });
  });
});
