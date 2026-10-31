import { describe, expect, it } from "vitest";
import type { BatchItem } from "./batch";
import { summarizeBatch } from "./batch";

function item(
  name: string,
  state: BatchItem["state"],
  options: Partial<Pick<BatchItem, "lines" | "reveals" | "headline" | "skip" | "format" | "hasRemovableMetadata">> = {},
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
      damaged: 0,
      incomplete: 1,
      hidden: 1,
      flagged: 2,
      dangerous: true,
      chosen: 0,
      problemCount: 2,
      anyPick: true,
    });
    expect(result.shown.map(({ index }) => index)).toEqual([1, 2]);
  });

  it("includes unknown and partially inspected files in the attention filter", () => {
    const unknown = item("unknown.bin", "done", { lines: [{ kind: "unknown", text: "Not read", node: 0 }] });
    const partial = item("partial.mp3", "done", { lines: [{ kind: "warning", text: "Partial inspection", node: 0 }] });
    const result = summarizeBatch([unknown, partial, item("ok.jpg", "done")], "problems");
    expect(result).toMatchObject({ incomplete: 2, damaged: 0, problemCount: 2, flagged: 2 });
    expect(result.shown.map(({ index }) => index)).toEqual([0, 1]);
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

  it("offers a clean copy for MP3 tags but not other read-only formats", () => {
    const files = ["mp3", "flac", "ogg", "eml", "msg", "unknown"].map((format) => item(
      `file.${format}`,
      "done",
      { format, reveals: ["author"], ...(format === "mp3" ? { hasRemovableMetadata: true } : {}) },
    ));
    const result = summarizeBatch(files, "reveals");
    expect(result).toMatchObject({ revealing: 6, chosen: 1, anyPick: true });
  });

  it("offers a tagged MP3 copy when every frame is outside the supported fact list", () => {
    const tagged = item("tagged.mp3", "done", {
      format: "mp3",
      lines: [{ kind: "warning", text: "Partial inspection", node: 0 }],
      hasRemovableMetadata: true,
    });
    const plain = item("plain.mp3", "done", { format: "mp3" });

    expect(summarizeBatch([tagged, plain], "all")).toMatchObject({ chosen: 1, anyPick: true });
  });

  it("does not offer an MP3 copy when the edge tag or audio frame did not pass the clean checks", () => {
    const unsafe = item("stacked.mp3", "done", {
      format: "mp3",
      reveals: ["artist"],
      hasRemovableMetadata: false,
    });

    expect(summarizeBatch([unsafe], "reveals")).toMatchObject({ chosen: 0, anyPick: false });
  });

  it("keeps only the visible page of row descriptors while counting every match", () => {
    const files = Array.from({ length: 20 }, (_, index) => item(`${index}.jpg`, "done"));

    const result = summarizeBatch(files, "all", 3);

    expect(result.shownCount).toBe(20);
    expect(result.shown.map(({ index }) => index)).toEqual([0, 1, 2]);
  });
});
