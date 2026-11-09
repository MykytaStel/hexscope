import type { BatchItem } from "./batch-model";
import { csvRow } from "./csv";

type IssueStatus = "read_failed" | "partial" | "unsupported" | "damaged" | "needs_attention";

function issueStatus(item: BatchItem): IssueStatus | null {
  if (item.state === "failed") return "read_failed";
  if (item.coverage === "partial") return "partial";
  if (item.coverage === "unsupported") return "unsupported";
  if (item.lines.some((line) => line.kind === "damage")) return "damaged";
  if (item.lines.some((line) => line.kind === "unknown" || line.kind === "warning")) return "needs_attention";
  return null;
}

/** A value-free list of batch rows that need attention, keyed only by position. */
export function batchDiagnosticsReport(items: readonly BatchItem[]): string | null {
  const issues = batchDiagnosticRows(items);
  if (issues.length === 0) return null;
  return JSON.stringify({
    schema: "hexscope.batch-diagnostics",
    version: 1,
    total: items.length,
    issues,
  }, null, 2);
}

/** A spreadsheet-friendly version of the same value-free issue list. */
export function batchDiagnosticsCsv(items: readonly BatchItem[]): string | null {
  const issues = batchDiagnosticRows(items);
  if (issues.length === 0) return null;
  return [
    csvRow(["Row", "Format", "Status", "Total files"]),
    ...issues.map(({ index, format, status }) => csvRow([index, format, status, items.length])),
  ].join("\r\n");
}

export interface BatchDiagnosticRow {
  /** One-based position in the current batch. */
  index: number;
  format: string;
  status: IssueStatus;
}

/** Local-only rows used to connect a private report entry back to its file. */
export function batchDiagnosticRows(items: readonly BatchItem[]): BatchDiagnosticRow[] {
  return items.flatMap((item, index) => {
    const status = issueStatus(item);
    return status ? [{ index: index + 1, format: item.format ?? "unknown", status }] : [];
  });
}
