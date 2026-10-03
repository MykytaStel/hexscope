import { describe, expect, it } from "vitest";
import { decodeVerification, verificationMessage } from "./verification";

describe("clean-copy verification reports", () => {
  it("decodes the versioned value-free report", () => {
    const report = decodeVerification(
      JSON.stringify({
        schema_version: 1,
        removed: [{ kind: "location", reason: "removed", unexpected: false }],
        present: [],
        unchecked: [{ kind: "lens", reason: "coverage_incomplete", unexpected: false }],
      }),
    );

    expect(report.removed[0]).toEqual({ kind: "location", reason: "removed", unexpected: false });
    expect(report.unchecked[0]).toEqual({ kind: "lens", reason: "coverage_incomplete", unexpected: false });
    expect(verificationMessage(report)).toBe(
      "Some findings were removed, but this copy is not fully checked yet.",
    );
    expect(JSON.stringify(report)).not.toMatch(/"value"|"scope"|camera A|private/);
  });

  it("turns malformed, unknown, or value-bearing reports into unchecked", () => {
    for (const raw of [
      "not json",
      JSON.stringify({ schema_version: 2, removed: [], present: [], unchecked: [] }),
      JSON.stringify({
        schema_version: 1,
        removed: [{ kind: "camera", reason: "removed", unexpected: false, value: "private" }],
        present: [],
        unchecked: [],
      }),
    ]) {
      const report = decodeVerification(raw);
      expect(report.removed).toEqual([]);
      expect(report.unchecked).toEqual([
        { kind: "file", reason: "verification_skipped", unexpected: false },
      ]);
    }
  });
});
