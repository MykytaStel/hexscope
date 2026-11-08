import { describe, expect, it } from "vitest";
import type { BatchItem } from "./batch-model";
import { batchDiagnosticsReport } from "./batch-diagnostics-report";

function item(overrides: Partial<BatchItem>): BatchItem {
  return {
    file: new File(["private contents"], "secret-client-contract.pdf"),
    state: "done",
    kind: "PDF",
    lines: [],
    headline: null,
    reveals: [],
    cleanName: "secret-client-contract-clean.pdf",
    format: "pdf",
    coverage: "complete",
    note: "private raw parser error with a path /Users/alice/work/client.pdf",
    skip: false,
    ...overrides,
  };
}

describe("batch diagnostics report", () => {
  it("identifies only affected rows by position, format, and a safe status", () => {
    const report = batchDiagnosticsReport([
      item({}),
      item({ format: "unknown", kind: "File", coverage: "unsupported" }),
      item({ state: "failed", format: undefined, coverage: undefined }),
      item({ coverage: "partial" }),
    ]);

    expect(JSON.parse(report!)).toEqual({
      schema: "hexscope.batch-diagnostics",
      version: 1,
      total: 4,
      issues: [
        { index: 2, format: "unknown", status: "unsupported" },
        { index: 3, format: "unknown", status: "read_failed" },
        { index: 4, format: "pdf", status: "partial" },
      ],
    });
    expect(report).not.toContain("secret-client-contract");
    expect(report).not.toContain("private contents");
    expect(report).not.toContain("/Users/alice");
    expect(report).not.toContain("private raw parser error");
  });

  it("omits rows with complete reads and returns no report when there is nothing to diagnose", () => {
    expect(batchDiagnosticsReport([item({})])).toBeNull();
    expect(batchDiagnosticsReport([])).toBeNull();
  });
});
