import { el } from "./dom";
import { currentLocale, translateText } from "./i18n";
import type { FileModel } from "./model";
import { FACT_LABELS } from "./finding-kinds";
import { MAX_VERIFY_BYTES } from "./verification";
import { call } from "./rpc";
import { saveAs } from "./files";
import { checkedProfiles, differingFields, evidencePath, readProfile, type PublicationProfile } from "./preflight-model";
import "./preflight.css";

const say = (en: string, uk: string) => currentLocale() === "uk" ? uk : en;
const words = (kind: string) => translateText(FACT_LABELS[kind] ?? kind, currentLocale());

/** Optional tools, loaded only when their disclosure is opened. */
export function mountPreflight(host: HTMLElement, m: FileModel, select: (node: number) => void): void {
  host.classList.add("preflight-tools");
  const plan = el("section");
  plan.append(el("h3", undefined, say("Plan changes before making a copy", "План змін перед створенням копії")));
  const planned = m.file.plannedRemovalKinds ?? [];
  plan.append(el("p", "hint", say("Planned changes are based on supported removal capabilities. The writer may refuse this file; only a checked output can confirm removal.", "План спирається на підтримувані можливості очищення. Записувач може відмовити; видалення підтверджує лише перевірка готової копії.")));
  const list = el("ul");
  for (const kind of planned) list.append(el("li", undefined, words(kind)));
  if (!planned.length) list.append(el("li", undefined, say("No observed findings have a complete removal plan.", "Для знайдених даних немає повного плану підтверджуваного видалення.")));
  const remaining = [...new Set(m.file.facts.map((fact) => fact.kind))].filter((kind) => !planned.includes(kind));
  if (remaining.length) plan.append(el("p", undefined, `${say("Outside this plan", "Поза цим планом")}: ${remaining.map(words).join(", ")}`));
  plan.append(list, el("p", "hint", (m.source?.size ?? m.bytes.length) > MAX_VERIFY_BYTES
    ? say("This source exceeds the 10 MiB read-back limit. Verification may be skipped. For larger files use: hexscope clean --verify PATH", "Джерело перевищує ліміт повторного читання 10 MiB. Перевірку можуть пропустити. Для великих файлів: hexscope clean --verify PATH")
    : say("Read-back is available only if both source and output fit within 10 MiB and the checks finish.", "Повторне читання доступне, лише якщо джерело й копія не перевищують 10 MiB та перевірки завершаться.")));
  host.append(plan);

  const policy = el("section");
  policy.append(el("h3", undefined, say("Local publication profile", "Локальний профіль публікації")));
  policy.append(el("p", "hint", say("Rules inspect Core findings only. Unknown codes and QR rules remain unchecked; OCR and visible content are not evaluated.", "Правила перевіряють лише знахідки Core. Невідомі коди й правила QR залишаються неперевіреними; OCR і видимий вміст не оцінюються.")));
  const kinds = el("input");
  kinds.value = "location,serial,owner";
  kinds.setAttribute("aria-label", say("Blocked finding codes, comma separated", "Коди заборонених знахідок через кому"));
  const strict = el("input"); strict.type = "checkbox"; strict.checked = true;
  const label = el("label"); label.append(strict, say(" Require a complete format inspection", " Вимагати повного форматного аналізу"));
  const status = el("p", "hint"); status.setAttribute("role", "status");
  const profile = (): PublicationProfile => readProfile(JSON.stringify({ schema: "hexscope.policy", version: 1, blocked_kinds: kinds.value.split(",").map((kind) => kind.trim()).filter(Boolean), require_complete: strict.checked }));
  const check = el("button", "btn", say("Check these rules", "Перевірити ці правила"));
  check.onclick = async () => {
    check.disabled = true;
    checkedProfiles.delete(m.file);
    try {
      const rules = profile();
      const response = await call({ type: "policy", sourceToken: m.file.verificationToken ?? -1, kinds: rules.blocked_kinds.join(","), requireComplete: rules.require_complete }, 5000, {});
      if (response.type !== "policy") throw new Error(say("Reopen the file to check rules.", "Відкрийте файл знову, щоб перевірити правила."));
      const result = JSON.parse(response.result);
      if (JSON.stringify(profile()) !== JSON.stringify(rules)) return;
      checkedProfiles.set(m.file, rules);
      status.textContent = result.passed ? say("These rules passed. This is not a safety certification.", "Ці правила пройдено. Це не сертифікація безпеки.")
        : `${say("Rules need attention", "Правила потребують уваги")}: ${result.blocked_kinds.map(words).join(", ")}${result.unchecked ? say(" · inspection incomplete", " · аналіз неповний") : ""}`;
      for (const kind of result.blocked_kinds as string[]) {
        const fact = m.file.facts.find((item) => item.kind === kind && item.node >= 0 && item.node < m.file.parents.length);
        const node = fact?.node ?? (kind === "location" ? m.file.location?.node : undefined);
        if (node === undefined || node < 0 || node >= m.file.parents.length) continue;
        const link = el("button", "link", `${say("Show evidence", "Показати доказ")} · ${words(kind)}`);
        link.onclick = () => select(node);
        status.append(" ", link);
      }
    } catch (error) { status.textContent = String(error); }
    finally { check.disabled = false; }
  };
  const clearCheck = () => {
    checkedProfiles.delete(m.file);
    status.replaceChildren();
  };
  kinds.addEventListener("input", clearCheck);
  strict.addEventListener("change", clearCheck);
  const exportButton = el("button", "btn", say("Export profile", "Експортувати профіль"));
  exportButton.onclick = () => { try { saveAs("hexscope-policy.json", new Blob([JSON.stringify(profile(), null, 2)], { type: "application/json" })); } catch (error) { status.textContent = String(error); } };
  const input = el("input"); input.type = "file"; input.accept = ".json,application/json";
  input.setAttribute("aria-label", say("Import profile", "Імпортувати профіль"));
  input.onchange = async () => { try { const file = input.files?.[0]; if (!file) return; if (file.size > 65_536) throw new Error("Profile exceeds 64 KiB"); const rules = readProfile(await file.text()); kinds.value = rules.blocked_kinds.join(","); strict.checked = rules.require_complete; clearCheck(); } catch (error) { status.textContent = String(error); } };
  policy.append(kinds, label, check, exportButton, input, status);
  host.append(policy);

  const graph = el("details"); graph.append(el("summary", undefined, say("Finding origins and differing fields", "Походження знахідок і різні значення полів")));
  graph.append(el("p", "hint", say("Only stored tree relationships are shown. Different values may be normal; they do not prove tampering.", "Показано лише зв’язки дерева файла. Різні значення можуть бути нормальними й не доводять підробку.")));
  for (const fact of m.file.facts.slice(0, 80)) {
    const row = el("div", "preflight-path"); row.append(el("strong", undefined, words(fact.kind)));
    for (const node of evidencePath(m.file, fact.node)) {
      const link = el("button", "link", m.label(node)); link.onclick = () => select(node); row.append(link);
    }
    graph.append(row);
  }
  for (const field of differingFields(m.file)) {
    const row = el("p"); row.append(`${field.label}: `);
    for (const node of field.nodes) { const button = el("button", "link", m.value(node)); button.onclick = () => select(node); row.append(button, " · "); }
    graph.append(row);
  }
  host.append(graph);

  const lab = el("button", "btn", say("Try a damaged sample", "Спробувати пошкоджений зразок"));
  const labResult = el("p", "hint"); labResult.setAttribute("role", "status");
  lab.onclick = async () => {
    lab.disabled = true;
    try {
      const { runDamageLab } = await import("./sample-lab");
      const result = await runDamageLab();
      labResult.textContent = `${say("Sample problems before / after a CRC change", "Проблеми зразка до / після зміни CRC")}: ${result.before} / ${result.after}. ${say("Your file was not changed.", "Ваш файл не змінено.")}`;
    } catch (error) { labResult.textContent = String(error); }
    finally { lab.disabled = false; }
  };
  host.append(lab, labResult);
}
