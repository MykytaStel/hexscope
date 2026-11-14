import { describe, expect, it } from "vitest";
import { partitionMetadataRows } from "./knowledge";

describe("metadata priority", () => {
  it("keeps unredacted and hidden text ahead of secondary fields", () => {
    const rows = [
      { kind: "covered", label: "Redaction not applied" },
      { kind: "comments", label: "Comment author" },
      { kind: "hiddentext", label: "White-on-white text" },
      { kind: "attachments", label: "Embedded file" },
      { kind: "title", label: "Document title" },
    ];

    const { priority, secondary } = partitionMetadataRows(rows, (row) => row.kind);

    expect(priority.map((row) => row.kind)).toEqual(["covered", "hiddentext"]);
    expect(secondary.map((row) => row.kind)).toEqual(["comments", "attachments", "title"]);
    expect([...priority, ...secondary].map((row) => row.kind).sort()).toEqual(rows.map((row) => row.kind).sort());
  });
});
