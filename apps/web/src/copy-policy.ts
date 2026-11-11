import { el } from "./dom";
import { FACT_LABELS } from "./finding-kinds";
import { currentLocale, translateText } from "./i18n";
import type { PublicationProfile } from "./preflight-model";
import { call } from "./rpc";
import { MAX_VERIFY_BYTES } from "./verification";

export async function showCopyPolicy(status: HTMLElement, copy: Blob, profile: PublicationProfile): Promise<void> {
  const say = (en: string, uk: string) => currentLocale() === "uk" ? uk : en;
  const unchecked = say("Copy profile not checked", "Профіль для копії не перевірено");
  status.textContent = unchecked;
  if (copy.size > MAX_VERIFY_BYTES) {
    status.append(` · ${say("copy exceeds 10 MiB", "копія перевищує 10 МіБ")}`);
    return;
  }
  status.textContent = say("Checking copy against the publication profile…", "Перевіряємо копію за профілем публікації…");
  try {
    const result = await call({
      type: "copyPolicy",
      copy,
      kinds: profile.blocked_kinds.join(","),
      requireComplete: profile.require_complete,
    }, 15_000, {});
    if (result.type !== "policy") throw new Error();
    const report = JSON.parse(result.result);
    if (report.passed) {
      status.textContent = say("Copy passed the publication profile", "Копія пройшла перевірку за профілем публікації");
      return;
    }
    const kinds: string[] = report.blocked_kinds;
    status.textContent = kinds.length
      ? say("Copy still contains blocked findings", "У копії залишилися заборонені знахідки")
      : unchecked;
    if (kinds.length) {
      const list = el("ul");
      for (const kind of kinds) list.append(el("li", undefined, translateText(FACT_LABELS[kind] ?? kind, currentLocale())));
      status.append(list);
      if (report.unchecked) status.append(el("p", undefined, unchecked));
    }
  } catch {
    status.textContent = unchecked;
  }
}
