import { expect, it } from "vitest";
import { archiveIndex, leavesFolder, type ArchiveRow } from "./archive-index";

const names = (rows: ArchiveRow[]) => rows.map((r) => `${r.kind === "folder" ? "folder" : "entry"} ${r.name}`);
const indexes = (rows: ArchiveRow[]) => rows.map((r) => (r.kind === "entry" ? r.index : -1));

it("lists folders first, then files, each in name order", () => {
  const index = archiveIndex(["b.txt", "a/2.txt", "a/1.txt", "c/", "A.txt"]);
  const top = index.list("");
  expect(names(top)).toEqual(["folder a", "folder c", "entry A.txt", "entry b.txt"]);
  expect(top[0]).toMatchObject({ kind: "folder", path: "a/", entries: 2 });
  expect(top[1]).toMatchObject({ kind: "folder", path: "c/", entries: 0 });
  expect(names(index.list("a/"))).toEqual(["entry 1.txt", "entry 2.txt"]);
  expect(index.list("a/")[0]).toMatchObject({ kind: "entry", index: 2, path: "a/1.txt", leaves: false });
  expect(index.parent("a/")).toBe("");
  expect(index.parent("")).toBeNull();
  expect(index.entries).toBe(4);
  expect(index.folders).toBe(2);
});

it("keeps hostile names as they are and flags them", () => {
  const hostile = ["../../etc/cron.d/x", "/etc/passwd", "C:\\Windows\\x", "a\\b.txt", "a//b", "ok/fine.txt"];
  const index = archiveIndex(hostile);
  expect(names(index.list(""))).toEqual(["folder ..", "folder /", "folder a", "folder ok", "entry C:\\Windows\\x", "entry a\\b.txt"]);
  expect(hostile.map(leavesFolder)).toEqual([true, true, true, true, false, false]);
  expect(names(index.list("a/"))).toEqual(["folder /"]);
  expect(index.list("a//")[0]).toMatchObject({ kind: "entry", index: 4, name: "b", leaves: false });
  expect(index.list("../../etc/cron.d/")[0]).toMatchObject({ kind: "entry", index: 0, name: "x", leaves: true });
  expect(index.parent("/etc/")).toBe("/");
  expect(index.parent("/")).toBe("");
});

it("filters by path across folders, ignoring case, in archive order", () => {
  const index = archiveIndex(["Звіт-1.txt", "docs/звіт-2.txt", "docs/other.txt", "docs/"]);
  expect(indexes(index.filter("ЗВІТ"))).toEqual([0, 1]);
  expect(index.filter("")).toEqual([]);
  expect(indexes(index.filter("docs/"))).toEqual([1, 2]);
});

it("lists duplicate names twice", () => {
  expect(indexes(archiveIndex(["x.txt", "x.txt"]).list(""))).toEqual([0, 1]);
});

it("indexes a deeply nested path without exhausting the call stack", () => {
  const path = "a/".repeat(12_000);
  const index = archiveIndex([`${path}file.txt`]);
  expect(index.folders).toBe(12_000);
  expect(index.list("")[0]).toMatchObject({ kind: "folder", entries: 1 });
  expect(index.list(path)[0]).toMatchObject({ kind: "entry", name: "file.txt" });
});
