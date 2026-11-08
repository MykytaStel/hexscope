import { describe, expect, it } from "vitest";
import { batchCoverageReport } from "./batch-coverage-report";

describe("batch coverage report", () => {
  it("exports a versioned format summary without file names or findings", () => {
    const report = batchCoverageReport([
      { format: "png", files: 2, complete: 2, partial: 0, unsupported: 0 },
      { format: "jpeg", files: 3, complete: 2, partial: 1, unsupported: 0 },
    ], 5);

    expect(report).toBe(`{
  "schema": "hexscope.batch-coverage",
  "version": 1,
  "files": {
    "total": 5,
    "read": 5,
    "not_read": 0
  },
  "formats": [
    {
      "format": "jpeg",
      "files": 3,
      "complete": 2,
      "partial": 1,
      "unsupported": 0
    },
    {
      "format": "png",
      "files": 2,
      "complete": 2,
      "partial": 0,
      "unsupported": 0
    }
  ]
}`);
    expect(report).not.toContain("private-photo");
    expect(report).not.toContain("GPS");
    expect(report).not.toContain("path");
  });

  it("keeps failed inputs visible as an aggregate without naming them", () => {
    const report = JSON.parse(batchCoverageReport([
      { format: "jpeg", files: 1, complete: 1, partial: 0, unsupported: 0 },
    ], 2));

    expect(report.files).toEqual({ total: 2, read: 1, not_read: 1 });
  });
});
