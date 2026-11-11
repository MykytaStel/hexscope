import type { FileModel } from "./model";
import { el } from "./dom";
import { currentLocale, translateText } from "./i18n";

export function preflightLauncher(m: FileModel, select: (node: number) => void): HTMLElement {
  const tools = el("details", "preflight-disclosure");
  tools.append(el("summary", undefined, translateText("Before sharing: plan and tools", currentLocale())));
  let loaded = false;
  tools.addEventListener("toggle", async () => {
    if (!tools.open || loaded) return;
    loaded = true;
    try {
      const { mountPreflight } = await import("./preflight-view");
      const body = el("div"); mountPreflight(body, m, select); tools.append(body);
    } catch { loaded = false; }
  });
  return tools;
}
