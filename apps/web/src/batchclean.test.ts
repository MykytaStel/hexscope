import { afterEach, beforeEach, expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({
  failName: null as string | null,
  downloads: [] as { name: string; data: Blob }[],
  offerText: "",
  receiptButtonText: "",
  receiptButtonClick: null as (() => void) | null,
}));

vi.mock("./rpc", () => ({ call: vi.fn() }));
vi.mock("./files", () => ({
  phoneCanShare: () => false,
  saveAs: (name: string, data: Blob) => { state.downloads.push({ name, data }); },
}));
vi.mock("./zipwrite", () => ({
  StoredZip: class {
    private readonly entries: { name: string; contents: string }[] = [];

    async add(name: string, data: Blob): Promise<void> {
      if (state.failName === name) {
        state.failName = null;
        throw new Error("archive size limit reached");
      }
      this.entries.push({ name, contents: name === "hexscope-report.json" ? await data.text() : "clean copy" });
    }

    finish(): Blob {
      return new Blob([JSON.stringify(this.entries)]);
    }
  },
}));

import { cleanCopies } from "./batchclean";
import { call } from "./rpc";

function item(name: string, cleanName: string) {
  return {
    file: new File(["source"], name, { type: "image/jpeg" }),
    state: "done" as const,
    kind: "JPEG",
    lines: [],
    headline: null,
    reveals: ["location"],
    cleanName,
    note: "",
    skip: false,
  };
}

function clickReceiptButton(): void {
  if (!state.receiptButtonClick) throw new Error("receipt download button was not wired");
  state.receiptButtonClick();
}

beforeEach(() => {
  state.failName = "failed-clean.jpg";
  state.downloads = [];
  vi.mocked(call).mockResolvedValue({
    id: 1,
    type: "cleaned",
    copy: new Blob(["clean"]),
    removed: [],
    orientation: 1,
    error: "",
    verification: JSON.stringify({ schema_version: 1, removed: [], present: [], unchecked: [] }),
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

it("records one terminal batch receipt row when a ZIP copy write fails", async () => {
  const view = { result: "", offer: () => {}, render: () => {} };
  await cleanCopies([item("failed.jpg", "failed-clean.jpg"), item("saved.jpg", "saved-clean.jpg")], false, view, () => true);

  const archive = state.downloads.find(({ name }) => name === "hexscope-clean-copies.zip");
  expect(archive).toBeDefined();
  const entries = JSON.parse(await archive!.data.text()) as { name: string; contents: string }[];
  expect(entries.map(({ name }) => name)).toEqual(["saved-clean.jpg", "hexscope-report.json"]);
  const receipt = JSON.parse(entries[1].contents);
  expect(receipt).toMatchObject({ total: 2, written: 1, failed: 1 });
  expect(receipt.files).toHaveLength(2);
  expect(receipt.files).toEqual(expect.arrayContaining([
    expect.objectContaining({ index: 1, operation_state: "not_created" }),
    expect.objectContaining({ index: 2, operation_state: "written" }),
  ]));
});

it("keeps completed copies and exports the receipt separately if it cannot fit in the ZIP", async () => {
  state.failName = "hexscope-report.json";
  state.offerText = "";
  state.receiptButtonText = "";
  state.receiptButtonClick = null;
  vi.stubGlobal("localStorage", { getItem: () => null });
  vi.stubGlobal("document", {
    createElement: () => ({
      classList: { add: () => {} },
      set textContent(text: string) { state.receiptButtonText = text; },
      addEventListener: (_type: string, listener: () => void) => { state.receiptButtonClick = listener; },
    }),
  } as unknown as Document);
  const view = { result: "", offer: (text: string) => { state.offerText = text; }, render: () => {} };

  await cleanCopies([item("saved.jpg", "saved-clean.jpg")], false, view, () => true);

  expect(state.downloads.map(({ name }) => name)).toEqual(["hexscope-clean-copies.zip"]);
  const archive = state.downloads.find(({ name }) => name === "hexscope-clean-copies.zip")!;
  const entries = JSON.parse(await archive.data.text()) as { name: string; contents: string }[];
  expect(entries.map(({ name }) => name)).toEqual(["saved-clean.jpg"]);
  expect(state.offerText).toContain("receipt");
  expect(state.receiptButtonText).toBe("Save receipt as JSON");
  clickReceiptButton();
  const receipt = JSON.parse(await state.downloads.find(({ name }) => name === "hexscope-report.json")!.data.text());
  expect(receipt).toMatchObject({ total: 1, written: 1, failed: 0 });
});
