import type { VerificationItem, VerificationReasonCode, VerificationReport } from "./verification";
import { MAX_VERIFY_BYTES } from "./verification";

declare const __VERSION__: string;

const SCHEMA = "hexscope.clean-copy-receipt";
const FORMATS = new Set([
  "jpeg", "png", "heif", "webp", "gif", "tiff", "video", "pdf", "zip", "wasm", "eml", "cfb",
  "msg", "office97", "rtf", "svg", "unknown",
]);

export interface CleanCopyReceiptItem {
  kind: string;
  reason: VerificationReasonCode;
  unexpected: boolean;
}

export interface CleanCopyReceipt {
  schema: typeof SCHEMA;
  version: 1;
  hexscope_version: string;
  copy: { format: string; sha256: string | null; fingerprint_reason?: "size_limit" };
  verification: {
    schema_version: 1;
    removed: CleanCopyReceiptItem[];
    present: CleanCopyReceiptItem[];
    unchecked: CleanCopyReceiptItem[];
  };
}

const GROUP_REASONS = {
  removed: new Set<VerificationReasonCode>(["removed"]),
  present: new Set<VerificationReasonCode>(["still_present", "value_changed_same_kind", "unexpected_output"]),
  unchecked: new Set<VerificationReasonCode>([
    "no_stable_value", "coverage_incomplete", "parse_incomplete", "no_comparable_findings", "verification_skipped",
  ]),
} as const;

function codeFor(item: VerificationItem, group: keyof typeof GROUP_REASONS): VerificationReasonCode {
  const fallback: Record<keyof typeof GROUP_REASONS, VerificationReasonCode> = {
    removed: "removed",
    present: item.unexpected ? "unexpected_output" : "still_present",
    unchecked: "verification_skipped",
  };
  const code = item.reasonCode ?? fallback[group];
  if (!GROUP_REASONS[group].has(code)) throw new Error("The verification report is unsupported.");
  if (!/^[a-z][a-z0-9_-]{0,63}$/.test(item.kind)) throw new Error("The verification report is unsupported.");
  return code;
}

function receiptItems(items: VerificationItem[], group: keyof typeof GROUP_REASONS): CleanCopyReceiptItem[] {
  return items.map((item) => {
    const reason = codeFor(item, group);
    return { kind: item.kind, reason, unexpected: reason === "unexpected_output" };
  });
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** Build a portable, value-free receipt only after the user asks to save it. */
export async function makeCleanCopyReceipt(
  format: string,
  copy: Blob,
  report: VerificationReport,
): Promise<CleanCopyReceipt> {
  if (!FORMATS.has(format)) throw new Error("The output format is unsupported.");
  const verification = {
    schema_version: 1 as const,
    removed: receiptItems(report.removed, "removed"),
    present: receiptItems(report.present, "present"),
    unchecked: receiptItems(report.unchecked, "unchecked"),
  };

  if (copy.size > MAX_VERIFY_BYTES) {
    return {
      schema: SCHEMA,
      version: 1,
      hexscope_version: typeof __VERSION__ === "string" ? __VERSION__ : "dev",
      copy: { format, sha256: null, fingerprint_reason: "size_limit" },
      verification,
    };
  }

  if (!globalThis.crypto?.subtle) throw new Error("SHA-256 is not available in this browser.");
  const digest = await globalThis.crypto.subtle.digest("SHA-256", await copy.arrayBuffer());
  return {
    schema: SCHEMA,
    version: 1,
    hexscope_version: typeof __VERSION__ === "string" ? __VERSION__ : "dev",
    copy: { format, sha256: hex(new Uint8Array(digest)) },
    verification,
  };
}
