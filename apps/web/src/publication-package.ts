import { StoredZip } from "./zipwrite";
import { makeCleanCopyReceipt } from "./clean-copy-receipt";
import type { VerificationReport } from "./verification";

/** Fixed prose and counts only: source values and paths cannot enter HTML. */
export function publicationSummary(removed: number, present: number, unchecked: number): Blob {
  const count = (n: number) => Number.isSafeInteger(n) && n >= 0 ? n : 0;
  return new Blob([`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta http-equiv="Content-Security-Policy" content="default-src 'none'"><title>Hexscope copy summary</title><h1>Copy created</h1><p>Verification observations: ${count(removed)} removed; ${count(present)} present; ${count(unchecked)} not checked.</p><p>This report contains no source names or metadata values. Visible content is not cleared by metadata checks. A created copy is not a safety certification. Read the JSON receipt for details.</p></html>`], { type: "text/html" });
}

export async function makePublicationPackage(format: string, copy: Blob, report: VerificationReport, outputName?: string): Promise<Blob> {
  const receipt = await makeCleanCopyReceipt(format, copy, report);
  const zip = new StoredZip();
  const ext = outputName?.match(/\.([a-zA-Z0-9]{1,12})$/)?.[1].toLowerCase() ?? ({ jpeg: "jpg", heif: "heif", video: "mov", office97: "bin", cfb: "bin", unknown: "bin" } as Record<string, string>)[format] ?? format;
  await zip.add(`copy.${/^[a-z0-9]{1,12}$/.test(ext) ? ext : "bin"}`, copy);
  await zip.add("hexscope-receipt.json", new Blob([JSON.stringify(receipt, null, 2)], { type: "application/json" }));
  await zip.add("summary.html", publicationSummary(report.removed.length, report.present.length, report.unchecked.length));
  return zip.finish();
}
