import { el } from "./dom";
import { currentLocale, translateText } from "./i18n";
import type { VerificationReport } from "./verification";

const tr = (text: string) => translateText(text, currentLocale());

/** The small optional receipt control, loaded after copy verification resolves. */
export function appendCleanCopyReceiptAction(
  region: HTMLElement,
  format: string,
  copy: Blob,
  report: VerificationReport,
  outputName?: string,
): void {
  const button = el("button", "btn save-copy-receipt", tr("Save verification receipt"));
  button.onclick = () => {
    button.disabled = true;
    void import("./clean-copy-receipt-action")
      .then(({ saveCleanCopyReceipt }) => saveCleanCopyReceipt(button, format, copy, report))
      .catch(() => {
        button.textContent = tr("Try again");
        button.disabled = false;
      });
  };
  const pack = el("button", "btn", currentLocale() === "uk" ? "Зберегти копію зі звітом у ZIP" : "Save copy and report as ZIP");
  pack.onclick = async () => {
    pack.disabled = true;
    try {
      const [{ makePublicationPackage }, { saveAs }] = await Promise.all([import("./publication-package"), import("./files")]);
      saveAs("hexscope-publication.zip", await makePublicationPackage(format, copy, report, outputName));
    } catch { pack.textContent = tr("Try again"); }
    finally { pack.disabled = false; }
  };
  region.append(pack);
  region.append(
    el("p", "copy-receipt-note", tr("A receipt includes a stable SHA-256 fingerprint; it can be compared with a known copy.")),
    el("p", "copy-receipt-note", tr("Copies over 10 MiB have no fingerprint.")),
    button,
  );
}
