import type { BatchItem } from "./batch";
import { rasterModule } from "./film-core";
import { saveAs } from "./files";
import { registerLazyTranslations, currentLocale } from "./i18n";
import "./photo-tools.css";
export interface PhotoSignal { serial: string | null; location: [number, number] | null; taken: string | null }
interface MosaicReport { schema: string; version: number; total: number; serial_groups: number[][]; location_groups: number[][]; located_files: number; local_date_order: number[]; utc_date_order: number[] }
registerLazyTranslations({
  "Photo privacy mosaic": "Мозаїка приватності фото", "Close photo mosaic": "Закрити мозаїку фото", "Save mosaic report": "Зберегти звіт мозаїки",
  "This local view compares metadata already stored in the files. Matching facts do not prove identity, home, a route or film origin.": "Цей локальний перегляд порівнює метадані, уже записані у файлах. Збіги не доводять особу, місце проживання, маршрут або плівкове походження.",
  "Repeated stored serial values": "Повторювані записані серійні номери", "Stored GPS points within 100m of first file": "Записані GPS-точки до 100 м від першого файла",
  "Recorded dates without time zones": "Записані дати без часового поясу", "Recorded dates with normalized time zones": "Записані дати з нормалізованим часовим поясом",
  "Every group refers to file indexes. The saved JSON contains no filenames, coordinates, dates or serial values.": "Групи посилаються на номери файлів. JSON-звіт не містить назв файлів, координат, дат або серійних номерів.",
});
function el<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) { const e = document.createElement(tag); if (text) e.textContent = text; return e; }
export async function showMosaic(items: BatchItem[]): Promise<void> {
  const dialog = el("dialog"); dialog.className = "photo-tool photo-mosaic"; dialog.setAttribute("aria-labelledby", "photo-mosaic-title");
  const title = el("h2", "Photo privacy mosaic"); title.id = "photo-mosaic-title";
  const close = el("button", "Close"); close.className = "btn"; close.setAttribute("aria-label", "Close photo mosaic"); close.onclick = () => dialog.close();
  const info = el("p", "This local view compares metadata already stored in the files. Matching facts do not prove identity, home, a route or film origin.");
  const status = el("p", "Reading local photo signals…"); status.setAttribute("role", "status");
  dialog.append(close, title, info, status); document.body.append(dialog); dialog.showModal(); dialog.addEventListener("close", () => dialog.remove());
  try {
    if (items.length > 1000) throw new Error("Photo mosaic is limited to 1000 files.");
    const signals = items.flatMap((item, index) => item.photoSignal ? [{ index: index + 1, ...item.photoSignal }] : []);
    const m = await rasterModule();
    if (!dialog.open) return;
    const report = JSON.parse(m.photoMosaic(JSON.stringify(signals))) as MosaicReport;
    status.textContent = currentLocale() === "uk" ? `Перевірено ${report.total} / ${items.length} файлів · ${items.length - report.total} не перевірено · ${report.located_files} записаних GPS-точок` : `${report.total} / ${items.length} files checked for photo metadata · ${items.length - report.total} not checked · ${report.located_files} stored GPS points`;
    const blocks: [string, number[][]][] = [["Repeated stored serial values", report.serial_groups], ["Stored GPS points within 100m of first file", report.location_groups], ["Recorded dates without time zones", report.local_date_order.length > 1 ? [report.local_date_order] : []], ["Recorded dates with normalized time zones", report.utc_date_order.length > 1 ? [report.utc_date_order] : []]];
    for (const [label, groups] of blocks) {
      dialog.append(el("h3", label));
      if (!groups.length) { dialog.append(el("p", "No comparable group found.")); continue; }
      const list = el("ul"); list.className = "photo-tool-results";
      for (const group of groups) { const row = el("li"); row.textContent = group.map((index) => `${index}: ${items[index - 1]?.file.name ?? "file"}`).join(" → "); list.append(row); }
      dialog.append(list);
    }
    const save = el("button", "Save mosaic report"); save.className = "btn btn-primary";
    const notChecked = items.flatMap((item, index) => item.photoSignal ? [] : [index + 1]);
    save.onclick = () => saveAs("hexscope-photo-mosaic.json", new Blob([JSON.stringify({ ...report, input_total: items.length, not_checked_indexes: notChecked }, null, 2)], { type: "application/json" }));
    dialog.append(el("p", "Every group refers to file indexes. The saved JSON contains no filenames, coordinates, dates or serial values."), save);
  } catch (e) { if (dialog.open) status.textContent = e instanceof Error ? e.message : String(e); }
}
