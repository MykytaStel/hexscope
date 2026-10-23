import { saveAs } from "./files";
import { currentLocale, translateText } from "./i18n";
import { makeCleanCopyReceipt } from "./clean-copy-receipt";
import type { VerificationReport } from "./verification";

/** Loaded only after the person asks to save the optional receipt. */
export async function saveCleanCopyReceipt(
  button: HTMLButtonElement,
  format: string,
  copy: Blob,
  report: VerificationReport,
): Promise<void> {
  const tr = (text: string) => translateText(text, currentLocale());
  button.textContent = tr("Preparing receipt…");
  try {
    const receipt = await makeCleanCopyReceipt(format, copy, report);
    const json = new Blob([JSON.stringify(receipt, null, 2)], { type: "application/json" });
    // The filename is fixed; no source or output name enters the receipt.
    saveAs("hexscope-clean-copy.json", json);
    button.textContent = tr(receipt.copy.sha256 === null
      ? "Verification receipt saved without a fingerprint"
      : "Verification receipt saved");
  } catch {
    button.textContent = tr("Could not save verification receipt");
    button.disabled = false;
  }
}
