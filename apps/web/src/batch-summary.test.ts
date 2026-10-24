import { describe, expect, it } from "vitest";
import type { BatchItem } from "./batch";
import { summarizeBatch } from "./batch";

function item(
  name: string,
  state: BatchItem["state"],
  options: Partial<Pick<BatchItem, "lines" | "reveals" | "headline" | "skip">> = {},
): BatchItem {
  return {
    file: new File([name], name),
    state,
    kind: "JPEG",
    lines: [],
    headline: null,
    reveals: [],
    cleanName: name,
    note: "",
    skip: false,
    ...options,
  };
}

describe("batch summary", () => {
  it("counts statuses and filters rows in one consistent pass", () => {
    const waiting = item("waiting.jpg", "waiting");
    const hidden = item("hidden.pdf", "done", {
      lines: [{ kind: "hidden", text: "Hidden text", node: 1 }],
      reveals: ["author"],
      headline: { tone: "warning", text: "Check this file" },
      skip: true,
    });
    const broken = item("broken.png", "failed");

    const result = summarizeBatch([waiting, hidden, broken], "problems");

    expect(result).toMatchObject({
      settled: 2,
      busy: true,
      revealing: 1,
      damaged: 1,
      hidden: 1,
      flagged: 2,
      dangerous: true,
      chosen: 0,
      problemCount: 2,
      anyPick: true,
    });
    expect(result.shown.map(({ index }) => index)).toEqual([1, 2]);
  });

  it("counts only unskipped cleanable files as chosen", () => {
    const selected = item("selected.jpg", "done", { reveals: ["location"] });
    const skipped = item("skipped.jpg", "done", { reveals: ["camera"], skip: true });
    const safe = item("safe.jpg", "done");

    expect(summarizeBatch([selected, skipped, safe], "reveals")).toMatchObject({
      busy: false,
      revealing: 2,
      chosen: 1,
      problemCount: 0,
      anyPick: true,
    });
  });

  it("keeps only the visible page of row descriptors while counting every match", () => {
    const files = Array.from({ length: 20 }, (_, index) => item(`${index}.jpg`, "done"));

    const result = summarizeBatch(files, "all", 3);

    expect(result.shownCount).toBe(20);
    expect(result.shown.map(({ index }) => index)).toEqual([0, 1, 2]);
  });
});
