import { verifyCleanCopyReceipt, type ReceiptCheck } from "./clean-copy-receipt";
import { dismissable } from "./dialogs";
import { el } from "./dom";
import { currentLocale } from "./i18n";

export function showReceiptVerification(): void {
  const say = (en: string, uk: string) => currentLocale() === "uk" ? uk : en;
  const title = say("Verify a copy receipt", "Перевірити квитанцію копії");
  const dialog = el("dialog", "report");
  dialog.setAttribute("aria-label", title);
  const copy = el("input");
  copy.type = "file";
  const copyLabel = el("label");
  copyLabel.append(say("Copy file", "Файл копії"), " ", copy);
  const receipt = el("input");
  receipt.type = "file";
  receipt.accept = ".json,application/json";
  const receiptLabel = el("label");
  receiptLabel.append(say("Receipt JSON", "Квитанція JSON"), " ", receipt);
  const status = el("p");
  status.setAttribute("role", "status");
  const check = el("button", "btn btn-primary", say("Check fingerprint", "Звірити відбиток"));
  check.disabled = true;
  const messages: Record<ReceiptCheck, string> = {
    match: say("Copy bytes match the receipt.", "Байти копії відповідають квитанції."),
    mismatch: say("Copy bytes differ from the receipt.", "Байти копії відрізняються від квитанції."),
    invalid: say("Malformed or unsupported receipt.", "Пошкоджена або непідтримувана квитанція."),
    "no-fingerprint": say("The receipt has no copy fingerprint.", "Квитанція не містить відбитка копії."),
    "copy-too-large": say("This copy exceeds 10 MiB. Use hexscope receipt verify in the CLI.", "Копія перевищує 10 МіБ. Використайте hexscope receipt verify у CLI."),
  };
  const refresh = () => {
    check.disabled = !copy.files?.[0] || !receipt.files?.[0];
  };
  const clear = () => {
    status.textContent = "";
    refresh();
  };
  copy.onchange = receipt.onchange = clear;
  check.onclick = async () => {
    const file = copy.files?.[0];
    const record = receipt.files?.[0];
    if (!file || !record) return;
    copy.disabled = receipt.disabled = check.disabled = true;
    status.textContent = say("Checking…", "Перевіряємо…");
    try {
      status.textContent = messages[await verifyCleanCopyReceipt(file, record)];
    } catch {
      status.textContent = say("Fingerprint check could not finish.", "Не вдалося завершити перевірку відбитка.");
    } finally {
      copy.disabled = receipt.disabled = false;
      refresh();
    }
  };
  dialog.append(el("h2", undefined, title), copyLabel, receiptLabel, check, status);
  dismissable(dialog);
  dialog.addEventListener("close", () => dialog.remove(), { once: true });
  document.body.append(dialog);
  dialog.showModal();
}
