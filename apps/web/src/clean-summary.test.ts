import { describe, expect, it } from "vitest";
import { cleanCopySummary, confirmedRemovedKinds } from "./clean-summary";

describe("clean copy summary", () => {
  it("keeps removed, present, and unchecked findings distinct", () => {
    const summary = cleanCopySummary({
      removed: [
        { kind: "location", label: "Location" },
        { kind: "camera", label: "Camera" },
        { kind: "serial", label: "Serial number" },
        { kind: "owner", label: "Owner" },
      ],
      present: [],
      unchecked: [
        { kind: "lens", label: "Lens" },
        { kind: "date", label: "Date" },
        { kind: "software", label: "Software" },
        { kind: "thumbnail", label: "Thumbnail" },
      ],
    });

    expect(summary).toEqual({ state: "checked", removed: 4, present: 0, unchecked: 4, clear: false });
  });

  it("marks a copy clear only when readback found nothing and checked every finding", () => {
    expect(cleanCopySummary({ removed: [], present: [], unchecked: [] })).toEqual({
      state: "checked",
      removed: 0,
      present: 0,
      unchecked: 0,
      clear: true,
    });
    expect(cleanCopySummary({ removed: [], present: [{ kind: "camera", label: "Camera" }], unchecked: [] }).clear).toBe(false);
  });

  it("does not claim a clean result when verification is unavailable", () => {
    expect(cleanCopySummary(undefined)).toEqual({
      state: "unverified",
      removed: 0,
      present: 0,
      unchecked: 0,
      clear: false,
    });
  });

  it("only crosses out a kind when none of its findings remain unchecked or present", () => {
    const report = {
      removed: [
        { kind: "camera", label: "Camera" },
        { kind: "serial", label: "Serial number" },
      ],
      present: [{ kind: "camera", label: "Camera" }],
      unchecked: [{ kind: "serial", label: "Serial number" }],
    };

    expect(confirmedRemovedKinds(report)).toEqual([]);
    expect(confirmedRemovedKinds({ ...report, unchecked: [] })).toEqual(["serial"]);
    expect(confirmedRemovedKinds(undefined)).toEqual([]);
  });
});
