import type { CleanCopyReceipt } from "./clean-copy-receipt";

export interface ReceiptItem { kind: string; reason: string; unexpected: boolean }
export interface ReceiptRow {
  index: number;
  operation_state: "written" | "not_created";
  verification: { schema_version: 1; removed: ReceiptItem[]; present: ReceiptItem[]; unchecked: ReceiptItem[] };
}

const REASONS = {
  removed: new Set(["removed"]),
  present: new Set(["still_present", "value_changed_same_kind", "unexpected_output"]),
  unchecked: new Set(["no_stable_value", "coverage_incomplete", "parse_incomplete", "no_comparable_findings", "verification_skipped"]),
} as const;

export function receiptRow(index: number, state: ReceiptRow["operation_state"], core?: string): ReceiptRow {
  const skipped = { schema_version: 1 as const, removed: [], present: [], unchecked: [{ kind: "file", reason: "verification_skipped", unexpected: false }] };
  let verification: ReceiptRow["verification"] = skipped;
  try {
    const r = JSON.parse(core ?? "null");
    if (r?.schema_version !== 1) throw new Error("report unavailable");
    const group = (items: unknown, name: keyof typeof REASONS): ReceiptItem[] => {
      if (!Array.isArray(items)) throw new Error("invalid report");
      return items.map((i) => {
        if (
          typeof i?.kind !== "string"
          || !/^[a-z][a-z0-9_-]{0,63}$/.test(i.kind)
          || typeof i?.reason !== "string"
          || !REASONS[name].has(i.reason)
          || typeof i?.unexpected !== "boolean"
          || i.unexpected !== (i.reason === "unexpected_output")
        ) throw new Error("invalid item");
        return { kind: i.kind, reason: i.reason, unexpected: i.unexpected };
      });
    };
    verification = {
      schema_version: 1,
      removed: group(r.removed, "removed"),
      present: group(r.present, "present"),
      unchecked: group(r.unchecked, "unchecked"),
    };
  } catch { /* A missing bounded check is never a clear result. */ }
  return { index, operation_state: state, verification };
}
export interface BatchReceipt {
  schema: "hexscope.batch-receipt";
  version: 1;
  total: number;
  written: number;
  failed: number;
  removed: number;
  present: number;
  unchecked: number;
  files: ReceiptRow[];
}

export interface CopyReceiptRow {
  index: number;
  operation_state: "written" | "not_created";
  receipt: CleanCopyReceipt | null;
  fingerprint_reason?: "unavailable";
}

export type BatchReceiptCounts = Pick<BatchReceipt, "total" | "written" | "failed" | "removed" | "present" | "unchecked">;

export function copyReceiptRow(
  index: number,
  state: CopyReceiptRow["operation_state"],
  receipt?: CleanCopyReceipt,
): CopyReceiptRow {
  return {
    index,
    operation_state: state,
    receipt: state === "written" ? receipt ?? null : null,
    ...(state === "written" && !receipt ? { fingerprint_reason: "unavailable" as const } : {}),
  };
}

export function batchReceipt(files: ReceiptRow[]): BatchReceipt {
  return {
    schema: "hexscope.batch-receipt", version: 1, total: files.length,
    written: files.filter((r) => r.operation_state === "written").length,
    failed: files.filter((r) => r.operation_state === "not_created").length,
    removed: files.reduce((n, r) => n + r.verification.removed.length, 0),
    present: files.reduce((n, r) => n + r.verification.present.length, 0),
    unchecked: files.reduce((n, r) => n + r.verification.unchecked.length, 0), files,
  };
}

/** Opt-in schema: the default v1 shape above remains byte-for-byte unchanged. */
export function batchCleanCopyReceipt(files: CopyReceiptRow[], counts?: BatchReceiptCounts) {
  const totals = counts ?? {
    total: files.length,
    written: files.filter((row) => row.operation_state === "written").length,
    failed: files.filter((row) => row.operation_state === "not_created").length,
    removed: files.reduce((n, row) => n + (row.receipt?.verification.removed.length ?? 0), 0),
    present: files.reduce((n, row) => n + (row.receipt?.verification.present.length ?? 0), 0),
    unchecked: files.reduce((n, row) => n + (row.receipt?.verification.unchecked.length ?? 0), 0),
  };
  return { schema: "hexscope.batch-clean-copy-receipt" as const, version: 1 as const, ...totals, files };
}
