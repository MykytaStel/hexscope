import { describe, expect, it, vi } from "vitest";
import type { BatchItem } from "./batch";
import { BatchController, type BatchAnalysis, type BatchViewPort } from "./batch-controller";

class MemoryBatchView implements BatchViewPort {
  result = "";
  readonly snapshots: string[][] = [];

  render(items: BatchItem[]): void {
    this.snapshots.push(items.map((item) => item.state));
  }

  offer(text: string, ..._actions: HTMLButtonElement[]): void {
    this.result = text;
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function report(file: File): BatchAnalysis {
  return {
    kind: "JPEG",
    lines: [],
    headline: { tone: "ok", text: `Nothing personal found in ${file.name}` },
    reveals: [],
    cleanName: file.name,
  };
}

const file = (name: string) => new File([name], name);

describe("BatchController", () => {
  it("waits while a file is open, then analyzes each waiting file once after resume", async () => {
    const view = new MemoryBatchView();
    const second = deferred<BatchAnalysis>();
    const analyzed: string[] = [];
    const openFile = vi.fn();
    const controller = new BatchController(view, {
      analyze: (input) => {
        analyzed.push(input.name);
        return input.name === "two.jpg" ? second.promise : Promise.resolve(report(input));
      },
      openFile,
      saveClean: async () => {},
      announce: () => {},
    });

    controller.start([file("one.jpg"), file("two.jpg"), file("three.jpg")]);
    controller.resume();
    await vi.waitFor(() => expect(controller.items?.[1].state).toBe("reading"));
    expect(controller.items?.[0].state).toBe("done");

    controller.open(0);
    expect(openFile).toHaveBeenCalledOnce();
    second.resolve(report(file("two.jpg")));
    await vi.waitFor(() => expect(controller.items?.[1].state).toBe("done"));
    expect(analyzed).toEqual(["one.jpg", "two.jpg"]);

    controller.resume();
    await vi.waitFor(() => expect(controller.items?.map((item) => item.state)).toEqual(["done", "done", "done"]));
    expect(analyzed).toEqual(["one.jpg", "two.jpg", "three.jpg"]);
  });

  it("keeps a failed row and continues with the remaining files", async () => {
    const view = new MemoryBatchView();
    const announce = vi.fn();
    const controller = new BatchController(view, {
      analyze: async (input) => {
        if (input.name === "bad.jpg") throw new Error("could not read it");
        return report(input);
      },
      openFile: () => {},
      saveClean: async () => {},
      announce,
    });

    controller.start([file("bad.jpg"), file("good.jpg")]);
    controller.resume();
    await vi.waitFor(() => expect(controller.items?.map((item) => item.state)).toEqual(["failed", "done"]));

    expect(controller.items?.[0].note).toBe("could not read it");
    expect(view.snapshots.at(-1)).toEqual(["failed", "done"]);
    expect(announce).toHaveBeenCalledOnce();
  });

  it("does not redraw or announce a list cleared during analysis", async () => {
    const view = new MemoryBatchView();
    const pending = deferred<BatchAnalysis>();
    const announce = vi.fn();
    const controller = new BatchController(view, {
      analyze: () => pending.promise,
      openFile: () => {},
      saveClean: async () => {},
      announce,
    });

    controller.start([file("slow.jpg")]);
    controller.resume();
    const oldItems = controller.items;
    await vi.waitFor(() => expect(oldItems?.[0].state).toBe("reading"));
    controller.clear();
    const renderedAtClear = view.snapshots.length;
    pending.resolve(report(file("slow.jpg")));
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(controller.items).toBeNull();
    expect(oldItems?.[0].state).toBe("reading");
    expect(view.snapshots).toHaveLength(renderedAtClear);
    expect(announce).not.toHaveBeenCalled();
  });

  it("marks clean-copy work stale when its batch is cleared", async () => {
    const view = new MemoryBatchView();
    const pending = deferred<void>();
    let isCurrent: (() => boolean) | undefined;
    const controller = new BatchController(view, {
      analyze: async (input) => report(input),
      openFile: () => {},
      saveClean: async (_items, _keepNames, _output, current) => {
        isCurrent = current;
        await pending.promise;
        if (current()) view.result = "saved";
      },
      announce: () => {},
    });
    controller.start([file("one.jpg")]);

    const saving = controller.saveClean(false);
    await vi.waitFor(() => expect(isCurrent).toBeDefined());
    expect(isCurrent?.()).toBe(true);
    controller.clear();
    expect(isCurrent?.()).toBe(false);
    pending.resolve();
    await saving;

    expect(view.result).toBe("");
  });
});
