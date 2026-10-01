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
    show: vi.fn(),
    select: vi.fn(),
    failed: vi.fn(),
    note: vi.fn(),
    remember: vi.fn(),
    afterOpen: vi.fn(),
    clearPlayer: vi.fn(),
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
