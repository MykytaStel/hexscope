import { describe, expect, it, vi } from "vitest";
import type { FileModel } from "./model";
import {
  WorkspaceController,
  type WorkspaceServices,
  type WorkspaceView,
} from "./workspace-controller";

function model(name: string): FileModel {
  const value = Object.create(Object.prototype) as FileModel;
  Object.defineProperty(value, "name", { value: name, enumerable: true });
  return value;
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

function rig(overrides: Partial<WorkspaceServices> = {}) {
  const services: WorkspaceServices = {
    parse: async (file) => model(file.name),
    openEntry: async (index) => model(`entry-${index}`),
    openBytes: async () => model("inside"),
    back: async () => {},
    ...overrides,
  };
  const view: WorkspaceView = {
    loading: vi.fn(),
    progress: vi.fn(),
    show: vi.fn(),
    select: vi.fn(),
    failed: vi.fn(),
    note: vi.fn(),
    remember: vi.fn(),
    afterOpen: vi.fn(),
    clearPlayer: vi.fn(),
    cancelled: vi.fn<WorkspaceView["cancelled"]>(),
  };
  let selected = -1;
  const controller = new WorkspaceController({
    services,
    view,
    maxFileBytes: 1024,
    getSelected: () => selected,
  });
  return { controller, services, view, select: (id: number) => (selected = id) };
}

const file = (name: string) => new File([name], name);

function fileModel(name: string, bytes: Uint8Array, source: Blob | null = null): FileModel {
  const value = model(name);
  Object.defineProperties(value, {
    bytes: { value: bytes },
    source: { value: source },
  });
  return value;
}

describe("WorkspaceController", () => {
  it("keeps the newest opened file when an earlier parse finishes later", async () => {
    const firstParse = deferred<FileModel>();
    const latest = model("latest.jpg");
    const latestFile = file("latest.jpg");
    const { controller, view } = rig({
      parse: (input) => (input.name === "slow.jpg" ? firstParse.promise : Promise.resolve(latest)),
    });

    const slow = controller.openFile(file("slow.jpg"));
    await controller.openFile(latestFile);
    firstParse.resolve(model("slow.jpg"));
    await slow;

    expect(controller.model).toBe(latest);
    expect(controller.levels).toEqual([]);
    expect(view.show).toHaveBeenCalledOnce();
    expect(view.show).toHaveBeenCalledWith(latest, []);
    expect(view.remember).toHaveBeenCalledWith(latestFile);
    expect(view.afterOpen).toHaveBeenCalledWith(latest);
  });

  it("aborts a stale parse when a newer file is opened", async () => {
    const firstParse = deferred<FileModel>();
    let firstSignal: AbortSignal | undefined;
    const latest = model("latest.jpg");
    const { controller } = rig({
      parse: (input, options) => {
        if (input.name === "slow.jpg") {
          firstSignal = options?.signal;
          return firstParse.promise;
        }
        return Promise.resolve(latest);
      },
    });

    const slow = controller.openFile(file("slow.jpg"));
    await controller.openFile(file("latest.jpg"));
    firstParse.resolve(model("slow.jpg"));
    await slow;

    expect(firstSignal?.aborted).toBe(true);
    expect(controller.model).toBe(latest);
  });

  it("shows progress only for the file that is still opening", async () => {
    const pendingParse = deferred<FileModel>();
    let report: ((progress: { stage: "ranges"; loaded: number; total: number; cancellable: boolean }) => void) | undefined;
    const { controller, view } = rig({
      parse: (_input, options) => {
        report = options?.onProgress;
        return pendingParse.promise;
      },
    });

    const opening = controller.openFile(file("large.mov"));
    const progress = { stage: "ranges" as const, loaded: 10, total: 100, cancellable: true };
    report?.(progress);
    pendingParse.resolve(model("large.mov"));
    await opening;

    expect(view.progress).toHaveBeenCalledWith(progress);
  });

  it("lets a person cancel the current file open", async () => {
    const firstParse = deferred<FileModel>();
    let firstSignal: AbortSignal | undefined;
    const { controller, view } = rig({
      parse: (_input, options) => {
        firstSignal = options?.signal;
        return firstParse.promise;
      },
    });

    const opening = controller.openFile(file("slow.mov"));
    controller.cancelOpen();
    firstParse.reject(new Error("Reading cancelled"));
    await opening;

    expect(firstSignal?.aborted).toBe(true);
    expect(controller.isOpeningFile).toBe(false);
    expect(view.failed).toHaveBeenCalledWith("Reading was cancelled.");
  });

  it("restores the previous workspace when a new file read is cancelled", async () => {
    const pendingParse = deferred<FileModel>();
    const previousFile = file("previous.jpg");
    const previous = model("previous.jpg");
    const { controller, view } = rig({
      parse: (input) => input.name === previousFile.name ? Promise.resolve(previous) : pendingParse.promise,
    });

    await controller.openFile(previousFile);
    const opening = controller.openFile(file("slow.mov"));
    controller.cancelOpen();
    pendingParse.reject(new Error("Reading cancelled"));
    await opening;

    expect(controller.model).toBe(previous);
    expect(view.cancelled).toHaveBeenCalledOnce();
    expect(view.cancelled).toHaveBeenCalledWith(previous, []);
    expect(view.failed).not.toHaveBeenCalledWith("Reading was cancelled.");
  });

  it("restores the previous file when cancellation interrupts retry after a worker reset", async () => {
    const firstAttempt = deferred<FileModel>();
    const retry = deferred<FileModel>();
    const previousFile = file("previous.jpg");
    const previous = fileModel("previous.jpg", new Uint8Array([1]), previousFile);
    const restoredPrevious = fileModel("previous.jpg", new Uint8Array([1]), previousFile);
    let previousAttempts = 0;
    let requestedAttempts = 0;
    let retryStarted!: () => void;
    const retryHasStarted = new Promise<void>((resolve) => { retryStarted = resolve; });
    const { controller, view } = rig({
      parse: (input) => {
        if (input.name === "previous.jpg") return Promise.resolve(++previousAttempts === 1 ? previous : restoredPrevious);
        requestedAttempts++;
        if (requestedAttempts === 1) return firstAttempt.promise;
        retryStarted();
        return retry.promise;
      },
    });

    await controller.openFile(previousFile);
    const opening = controller.openFile(file("requested.jpg"));
    await controller.recoverAfterWorkerReset();
    firstAttempt.reject(new Error("the worker stopped after copy verification timed out"));
    await retryHasStarted;

    controller.cancelOpen();
    retry.reject(new Error("Reading cancelled"));
    await opening;
    await vi.waitFor(() => expect(controller.model).toBe(restoredPrevious));

    expect(previousAttempts).toBe(2);
    expect(view.failed).not.toHaveBeenCalledWith("Reading was cancelled.");
    expect(view.show).toHaveBeenLastCalledWith(restoredPrevious, []);
  });

  it("restores the current file from local bytes after the worker is reset", async () => {
    const rootFile = file("root.zip");
    const nestedBytes = new Uint8Array([0x50, 0x4b, 0x03, 0x04]);
    const root = fileModel("root.zip", new Uint8Array([1]), rootFile);
    const nested = fileModel("inside.docx", nestedBytes);
    const reopened = fileModel("inside.docx", nestedBytes);
    const parse = vi.fn(async (input: File) => input.name === "root.zip" ? root : reopened);
    const { controller, view } = rig({
      parse,
      openEntry: async () => nested,
    });

    await controller.openFile(rootFile);
    await controller.openEntry(0, "inside.docx");
    await controller.recoverAfterWorkerReset();

    const restoredFile = parse.mock.calls[1][0];
    expect(restoredFile.name).toBe("inside.docx");
    await expect(restoredFile.arrayBuffer()).resolves.toEqual(nestedBytes.buffer);
    expect(controller.model).toBe(reopened);
    expect(controller.levels).toEqual([]);
    expect(view.show).toHaveBeenLastCalledWith(reopened, []);
  });

  it("retries the requested file when the worker resets during its open", async () => {
    const firstAttempt = deferred<FileModel>();
    const previous = model("previous.jpg");
    const requested = model("requested.jpg");
    let requestedAttempts = 0;
    const parse = vi.fn(async (input: File) => {
      if (input.name === "previous.jpg") return previous;
      requestedAttempts++;
      return requestedAttempts === 1 ? firstAttempt.promise : requested;
    });
    const { controller, view } = rig({ parse });

    await controller.openFile(file("previous.jpg"));
    expect(controller.isOpeningFile).toBe(false);
    const opening = controller.openFile(file("requested.jpg"));
    expect(controller.isOpeningFile).toBe(true);
    await controller.recoverAfterWorkerReset();
    firstAttempt.reject(new Error("the reader stopped after copy verification timed out"));
    await opening;

    expect(controller.isOpeningFile).toBe(false);
    expect(requestedAttempts).toBe(2);
    expect(controller.model).toBe(requested);
    expect(view.failed).not.toHaveBeenCalled();
    expect(view.show).toHaveBeenLastCalledWith(requested, []);
  });

  it("does not retry the same file more than once after worker resets", async () => {
    const firstAttempt = deferred<FileModel>();
    const retry = deferred<FileModel>();
    let attempts = 0;
    const { controller, view } = rig({
      parse: () => ++attempts === 1 ? firstAttempt.promise : retry.promise,
    });

    const opening = controller.openFile(file("slow.jpg"));
    await controller.recoverAfterWorkerReset();
    firstAttempt.reject(new Error("first timeout"));
    await Promise.resolve();
    await controller.recoverAfterWorkerReset();
    retry.reject(new Error("second timeout"));
    await opening;

    expect(attempts).toBe(2);
    expect(view.failed).toHaveBeenCalledOnce();
    expect(view.failed).toHaveBeenCalledWith("Could not read slow.jpg: second timeout");
  });

  it("restores the previous file if the one retry after reset fails", async () => {
    const firstAttempt = deferred<FileModel>();
    const previous = model("previous.jpg");
    const restoredPrevious = model("previous.jpg");
    let previousAttempts = 0;
    let requestedAttempts = 0;
    const parse = vi.fn(async (input: File) => {
      if (input.name === "previous.jpg") {
        return ++previousAttempts === 1 ? previous : restoredPrevious;
      }
      if (++requestedAttempts === 1) return firstAttempt.promise;
      throw new Error("requested file is not readable");
    });
    const { controller, view } = rig({ parse });

    await controller.openFile(file("previous.jpg"));
    const opening = controller.openFile(file("requested.jpg"));
    await controller.recoverAfterWorkerReset();
    firstAttempt.reject(new Error("reader stopped after timeout"));
    await opening;

    expect(parse.mock.calls.map(([input]) => input.name)).toEqual([
      "previous.jpg",
      "requested.jpg",
      "requested.jpg",
      "previous.jpg",
    ]);
    expect(controller.model).toBe(restoredPrevious);
    expect(view.failed).toHaveBeenCalledWith("Could not read requested.jpg: requested file is not readable");
    expect(view.show).toHaveBeenLastCalledWith(restoredPrevious, []);
  });

  it("does not let a pending nested open replace a newer file", async () => {
    const root = model("root.zip");
    const latest = model("new.jpg");
    const pendingEntry = deferred<FileModel>();
    const { controller, view } = rig({
      parse: async (input) => (input.name === "root.zip" ? root : latest),
      openEntry: () => pendingEntry.promise,
    });

    await controller.openFile(file("root.zip"));
    const opening = controller.openEntry(0, "inside.jpg");
    await controller.openFile(file("new.jpg"));
    pendingEntry.resolve(model("inside.jpg"));
    await opening;

    expect(controller.model).toBe(latest);
    expect(controller.levels).toEqual([]);
    expect(view.show).toHaveBeenLastCalledWith(latest, []);
  });

  it("leaves the parent and its selection intact when a nested entry fails", async () => {
    const parent = model("parent.zip");
    const openEntry = vi.fn(async () => {
      throw new Error("entry cannot be opened");
    });
    const { controller, view, select } = rig({
      parse: async () => parent,
      openEntry,
    });
    await controller.openFile(file("parent.zip"));
    select(17);
    const shown = vi.mocked(view.show).mock.calls.length;

    await controller.openEntry(2, "broken.docx");

    expect(openEntry).toHaveBeenCalledWith(2, "broken.docx");
    expect(controller.model).toBe(parent);
    expect(controller.levels).toEqual([]);
    expect(view.show).toHaveBeenCalledTimes(shown);
    expect(view.note).toHaveBeenCalledWith("entry cannot be opened");
  });

  it("stores each parent selection and restores it from the requested breadcrumb", async () => {
    const root = model("root.zip");
    const nested = model("nested.zip");
    const inside = model("inside.jpg");
    const back = vi.fn(async () => {});
    const { controller, view, select } = rig({
      parse: async () => root,
      openEntry: async () => nested,
      openBytes: async () => inside,
      back,
    });
    await controller.openFile(file("root.zip"));
    select(7);
    await controller.openEntry(0, "nested.zip");
    expect(controller.levels).toEqual([{ model: root, selected: 7 }]);
    select(11);
    await controller.openInside(new Uint8Array([1, 2, 3]), "inside.jpg");

    expect(controller.levels).toEqual([
      { model: root, selected: 7 },
      { model: nested, selected: 11 },
    ]);
    await controller.back(0);

    expect(back).toHaveBeenCalledWith(0);
    expect(controller.model).toBe(root);
    expect(controller.levels).toEqual([]);
    expect(view.show).toHaveBeenLastCalledWith(root, []);
    expect(view.select).toHaveBeenCalledWith(7);
  });

  it("keeps the current file when the worker cannot restore a breadcrumb", async () => {
    const parent = model("parent.zip");
    const nested = model("nested.jpg");
    const { controller, view } = rig({
      parse: async () => parent,
      openEntry: async () => nested,
      back: async () => {
        throw new Error("worker is unavailable");
      },
    });
    await controller.openFile(file("parent.zip"));
    await controller.openEntry(0, "nested.jpg");

    await expect(controller.back(0)).resolves.toBeUndefined();

    expect(controller.model).toBe(nested);
    expect(controller.levels).toEqual([{ model: parent, selected: -1 }]);
    expect(view.note).toHaveBeenCalledWith("worker is unavailable");
  });

  it("passes display names to nested parsers so each model is indexed once", async () => {
    const parent = model("parent.zip");
    const entry = model("report.docx");
    const embedded = model("photo.jpg");
    const openEntry = vi.fn(async (_index: number, _name?: string) => entry);
    const openBytes = vi.fn(async (_bytes: Uint8Array, _name?: string) => embedded);
    const { controller } = rig({
      parse: async () => parent,
      openEntry,
      openBytes,
    });
    const bytes = new Uint8Array([1, 2, 3]);
    await controller.openFile(file("parent.zip"));
    await controller.openEntry(4, "report.docx");
    await controller.openInside(bytes, "photo.jpg");

    expect(openEntry).toHaveBeenCalledWith(4, "report.docx");
    expect(openBytes).toHaveBeenCalledWith(expect.any(Uint8Array), "photo.jpg");
    expect(openBytes.mock.calls[0][0]).not.toBe(bytes);
    expect(controller.model).toBe(embedded);
  });
});
