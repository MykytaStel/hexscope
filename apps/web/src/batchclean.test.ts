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
    format: "jpeg",
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

it("opts into copy fingerprints while failed rows never receive a receipt", async () => {
  state.failName = "failed-clean.jpg";
  const view = { result: "", offer: () => {}, render: () => {} };
  await cleanCopies(
    [item("failed.jpg", "failed-clean.jpg"), item("private-name.jpg", "private-name-clean.jpg")],
    false,
    view,
    () => true,
    true,
  );

  const archive = state.downloads.find(({ name }) => name === "hexscope-clean-copies.zip")!;
  const entries = JSON.parse(await archive.data.text()) as { name: string; contents: string }[];
  const reportText = entries.find(({ name }) => name === "hexscope-report.json")!.contents;
  const receipt = JSON.parse(reportText);
  const expectedDigest = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode("clean"))),
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("");
  expect(receipt.schema).toBe("hexscope.batch-clean-copy-receipt");
  expect(receipt.files[0]).toEqual({ index: 1, operation_state: "not_created", receipt: null });
  expect(receipt.files[1]).toMatchObject({
    index: 2,
    operation_state: "written",
    receipt: {
      schema: "hexscope.clean-copy-receipt",
      copy: { sha256: expectedDigest },
    },
  });
  expect(reportText).not.toContain("private-name.jpg");
});

it("records the original input indexes even when the same item object appears twice", async () => {
  state.failName = null;
  const duplicate = item("same.jpg", "same-clean.jpg");
  const view = { result: "", offer: () => {}, render: () => {} };
  await cleanCopies([duplicate, duplicate], false, view, () => true);

  const archive = state.downloads.find(({ name }) => name === "hexscope-clean-copies.zip")!;
  const entries = JSON.parse(await archive.data.text()) as { name: string; contents: string }[];
  const receipt = JSON.parse(entries.find(({ name }) => name === "hexscope-report.json")!.contents);
  expect(receipt.files.map((row: { index: number }) => row.index)).toEqual([1, 2]);
  expect(entries.map(({ name }) => name)).toEqual(["same-clean.jpg", "2-same-clean.jpg", "hexscope-report.json"]);
});

it("omits only the fingerprint for a copy above the measured browser cap", async () => {
  state.failName = null;
  vi.mocked(call).mockResolvedValueOnce({
    id: 1,
    type: "cleaned",
    copy: new Blob([new Uint8Array(10 * 1024 * 1024 + 1)]),
    removed: [],
    orientation: 1,
    error: "",
    verification: JSON.stringify({
      schema_version: 1,
      removed: [],
      present: [],
      unchecked: [{ kind: "file", reason: "verification_skipped", unexpected: false }],
    }),
  });
  const view = { result: "", offer: () => {}, render: () => {} };
  await cleanCopies([item("large.jpg", "large-clean.jpg")], false, view, () => true, true);

  const archive = state.downloads.find(({ name }) => name === "hexscope-clean-copies.zip")!;
  const entries = JSON.parse(await archive.data.text()) as { name: string; contents: string }[];
  const receipt = JSON.parse(entries.find(({ name }) => name === "hexscope-report.json")!.contents);
  expect(receipt.files[0].receipt.copy.sha256).toBeNull();
  expect(receipt.files[0].receipt.copy.fingerprint_reason).toBe("size_limit");
  expect(receipt.files[0].receipt.verification.unchecked).toEqual([
    { kind: "file", reason: "verification_skipped", unexpected: false },
  ]);
});
