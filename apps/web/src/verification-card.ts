import { el } from "./dom";
import { registerTranslations } from "./i18n";
import { verificationMessage, type VerificationItem, type VerificationReport } from "./verification";

registerTranslations({
  "Copy check": "Перевірка копії",
  "Clean-copy verification": "Перевірка очищеної копії",
  "Some findings are still present in the copy.": "У копії залишилися деякі знахідки.",
  "Some findings were removed, but this copy is not fully checked yet.": "Деякі дані видалено, але перевірку копії ще не завершено.",
  "This copy could not be fully checked yet.": "Не вдалося повністю перевірити копію.",
  "The checked findings were removed.": "Перевірені знахідки видалено.",
  "No comparable findings were checked.": "Не було даних, які можна було порівняти.",
  "Checked and removed": "Перевірено й видалено",
  "Still present": "Залишилося в копії",
  "Could not check": "Не вдалося перевірити",
  "Found only in the clean copy.": "Знайдено лише в очищеній копії.",
  "Not found in the copy.": "У копії не знайдено.",
  "The same finding remains.": "Та сама знахідка залишилася.",
  "A finding of this kind remains with a different value.": "Залишилася така сама категорія даних, але зі зміненим значенням.",
  "There was no stable value to compare.": "Немає стабільного значення для порівняння.",
  "This kind is not in verified coverage yet.": "Цю категорію даних іще не охоплює перевірка.",
  "The parser reported a warning or error.": "Під час розбору файла виникло попередження або помилка.",
  "A finding appears only in the copy.": "У копії з’явилася нова знахідка.",
  "No comparable findings were available.": "Не було даних, які можна порівняти.",
  "The check was skipped.": "Перевірку пропущено.",
});

const KIND_LABELS: Record<string, string> = {
  camera: "Camera",
  location: "Location",
  serial: "Serial number",
  owner: "Owner",
  lens: "Lens",
  taken: "Taken",
  software: "Software",
  thumbnail: "Thumbnail",
  file: "File",
};

function kindLabel(kind: string): string {
  return KIND_LABELS[kind] ?? kind.charAt(0).toUpperCase() + kind.slice(1);
}

function reasonLabel(item: VerificationItem): string {
  if (item.unexpected) return "Found only in the clean copy.";
  switch (item.reason) {
    case "removed":
      return "Not found in the copy.";
    case "still_present":
      return "The same finding remains.";
    case "value_changed_same_kind":
      return "A finding of this kind remains with a different value.";
    case "no_stable_value":
      return "There was no stable value to compare.";
    case "coverage_incomplete":
      return "This kind is not in verified coverage yet.";
    case "parse_incomplete":
      return "The parser reported a warning or error.";
    case "unexpected_output":
      return "A finding appears only in the copy.";
    case "no_comparable_findings":
      return "No comparable findings were available.";
    case "verification_skipped":
      return "The check was skipped.";
  }
}

/** An accessible summary containing kinds and reasons, never personal values. */
export function cleanVerificationCard(report: VerificationReport): HTMLElement {
  const card = el("section", "clean-verification");
  card.setAttribute("aria-label", "Clean-copy verification");
  card.append(el("h3", "clean-verification-title", "Copy check"));
  const status = el("p", "clean-verification-status", verificationMessage(report));
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  card.append(status);

  const groups: [keyof Pick<VerificationReport, "removed" | "present" | "unchecked">, string][] = [
    ["removed", "Checked and removed"],
    ["present", "Still present"],
    ["unchecked", "Could not check"],
  ];
  for (const [key, title] of groups) {
    const findings = report[key];
    if (findings.length === 0) continue;
    const group = el("div", "clean-verification-group");
    group.append(el("h4", undefined, title));
    const list = el("ul");
    for (const finding of findings) {
      const item = el("li");
      item.append(el("strong", undefined, kindLabel(finding.kind)), el("span", undefined, reasonLabel(finding)));
      list.append(item);
    }
    group.append(list);
    card.append(group);
  }
  return card;
}
