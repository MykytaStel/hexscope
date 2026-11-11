import { dismissable } from "./dialogs";
import { el } from "./dom";
import { currentLocale } from "./i18n";
import { readProfile, type PublicationProfile } from "./preflight-model";

export function showBatchPolicyDialog(apply: (profile: PublicationProfile) => boolean): void {
  const say = (en: string, uk: string) => currentLocale() === "uk" ? uk : en;
  const dialog = el("dialog", "report");
  dialog.setAttribute("aria-label", say("Batch publication profile", "Профіль для пакета"));
  const kinds = el("input");
  kinds.value = "location,serial,owner";
  const kindsLabel = el("label");
  kindsLabel.append(say("Blocked finding codes", "Коди заборонених знахідок"), " ", kinds);
  const strict = el("input");
  strict.type = "checkbox";
  strict.checked = true;
  const strictLabel = el("label");
  strictLabel.append(strict, " ", say("Require a complete inspection", "Вимагати повного аналізу"));
  const input = el("input");
  input.type = "file";
  input.accept = ".json,application/json";
  const importLabel = el("label");
  importLabel.append(say("Import profile JSON", "Імпортувати профіль JSON"), " ", input);
  const status = el("p", "hint");
  status.setAttribute("role", "status");
  const button = el("button", "btn btn-primary", say("Check batch", "Перевірити пакет"));
  input.onchange = async () => {
    const file = input.files?.[0];
    if (!file) return;
    button.disabled = true;
    try {
      if (file.size > 65_536) throw new Error("Profile exceeds 64 KiB");
      const profile = readProfile(await file.text());
      kinds.value = profile.blocked_kinds.join(",");
      strict.checked = profile.require_complete;
      status.textContent = "";
    } catch (error) {
      status.textContent = String(error);
    } finally {
      button.disabled = false;
    }
  };
  button.onclick = () => {
    try {
      const profile = readProfile(JSON.stringify({
        schema: "hexscope.policy", version: 1, require_complete: strict.checked,
        blocked_kinds: kinds.value.split(",").map((kind) => kind.trim()).filter(Boolean),
      }));
      if (apply(profile)) dialog.close();
      else status.textContent = say("Wait for the batch to finish reading.", "Дочекайтеся завершення читання пакета.");
    } catch (error) {
      status.textContent = String(error);
    }
  };
  dialog.append(el("h2", undefined, say("Batch publication profile", "Профіль для пакета")),
    el("p", "hint", say("Applying a profile reads the batch again.", "Застосування профілю запускає повторне читання пакета.")),
    kindsLabel, strictLabel, importLabel, button, status);
  dismissable(dialog);
  dialog.addEventListener("close", () => dialog.remove(), { once: true });
  document.body.append(dialog);
  dialog.showModal();
}
