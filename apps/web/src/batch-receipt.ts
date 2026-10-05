export interface ReceiptItem { kind: string; reason: string; unexpected: boolean }
export interface ReceiptRow {
  index: number;
  operation_state: "written" | "not_created";
  verification: { schema_version: 1; removed: ReceiptItem[]; present: ReceiptItem[]; unchecked: ReceiptItem[] };
}
export function receiptRow(index: number, state: ReceiptRow["operation_state"], core?: string): ReceiptRow {
  const skipped = { schema_version: 1 as const, removed: [], present: [], unchecked: [{ kind: "file", reason: "verification_skipped", unexpected: false }] };
  let verification: ReceiptRow["verification"] = skipped;
  try {
    const r = JSON.parse(core ?? "null");
    if (r?.schema_version !== 1) throw new Error("report unavailable");
    const group = (items: unknown): ReceiptItem[] => {
      if (!Array.isArray(items)) throw new Error("invalid report");
      return items.map((i) => { if (typeof i?.kind !== "string" || typeof i?.reason !== "string" || typeof i?.unexpected !== "boolean") throw new Error("invalid item"); return { kind: i.kind, reason: i.reason, unexpected: i.unexpected }; });
    };
    verification = { schema_version: 1, removed: group(r.removed), present: group(r.present), unchecked: group(r.unchecked) };
  } catch { /* A missing bounded check is never a clear result. */ }
  return { index, operation_state: state, verification };
}
export function batchReceipt(files: ReceiptRow[]) {
  return {
    schema: "hexscope.batch-receipt", version: 1, total: files.length,
    written: files.filter((r) => r.operation_state === "written").length,
    failed: files.filter((r) => r.operation_state === "not_created").length,
    removed: files.reduce((n, r) => n + r.verification.removed.length, 0),
    present: files.reduce((n, r) => n + r.verification.present.length, 0),
    unchecked: files.reduce((n, r) => n + r.verification.unchecked.length, 0), files,
  };
}
