import { cleanable, type BatchItem } from "./batch";
import { rasterModule } from "./film-core";
import { saveAs } from "./files";
import { registerLazyTranslations, currentLocale, translateText } from "./i18n";
import "./photo-tools.css";
export interface PhotoSignal { serial: string | null; location: [number, number] | null; taken: string | null }
interface MosaicReport { schema: string; version: number; total: number; serial_groups: number[][]; location_groups: number[][]; located_files: number; local_date_order: number[]; utc_date_order: number[] }
registerLazyTranslations({
  "Photo privacy mosaic": "Мозаїка приватності фото", "Close photo mosaic": "Закрити мозаїку фото", "Save mosaic report": "Зберегти звіт мозаїки",
  "Show this file in the batch list": "Показати цей файл у списку",
  "Select these photos for clean copies": "Вибрати ці фото для чистих копій",
  "No supported clean copies in this group": "Для цієї групи немає доступних чистих копій",
  "Selecting a group replaces the clean-copy selection. Review or change the checkboxes in the batch before saving.": "Вибір групи замінить поточний список чистих копій. Перед збереженням перевірте прапорці у списку файлів або змініть вибір.",
  "This local view compares metadata already stored in the files. Matching facts do not prove identity, home, a route or film origin.": "Цей локальний перегляд порівнює метадані, уже записані у файлах. Збіги не доводять особу, місце проживання, маршрут або плівкове походження.",
  "Repeated stored serial values": "Повторювані записані серійні номери", "Stored GPS points within 100m of first file": "Записані GPS-точки до 100 м від першого файла",
  "Recorded dates without time zones": "Записані дати без часового поясу", "Recorded dates with normalized time zones": "Записані дати з нормалізованим часовим поясом",
  "Every group refers to file indexes. The saved JSON contains no filenames, coordinates, dates or serial values.": "Групи посилаються на номери файлів. JSON-звіт не містить назв файлів, координат, дат або серійних номерів.",
});
function el<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) { const e = document.createElement(tag); if (text) e.textContent = text; return e; }
export async function showMosaic(
  items: BatchItem[],
  showFileInBatch: (index: number) => void,
  selectGroupForCleaning: (indexes: readonly number[]) => void,
): Promise<void> {
  const dialog = el("dialog"); dialog.className = "photo-tool photo-mosaic"; dialog.setAttribute("aria-labelledby", "photo-mosaic-title");
  const title = el("h2", "Photo privacy mosaic"); title.id = "photo-mosaic-title";
  const close = el("button", "Close"); close.className = "btn"; close.setAttribute("aria-label", "Close photo mosaic"); close.onclick = () => dialog.close();
  const info = el("p", "This local view compares metadata already stored in the files. Matching facts do not prove identity, home, a route or film origin.");
  const selectionNote = el("p", "Selecting a group replaces the clean-copy selection. Review or change the checkboxes in the batch before saving.");
  const status = el("p", "Reading local photo signals…"); status.setAttribute("role", "status");
  dialog.append(close, title, info, selectionNote, status); document.body.append(dialog); dialog.showModal(); dialog.addEventListener("close", () => dialog.remove());
  try {
    if (items.length > 1000) throw new Error("Photo mosaic is limited to 1000 files.");
    const signals = items.flatMap((item, index) => item.photoSignal ? [{ index: index + 1, ...item.photoSignal }] : []);
    const m = await rasterModule();
    if (!dialog.open) return;
    const report = JSON.parse(m.photoMosaic(JSON.stringify(signals))) as MosaicReport;
    const blocks: [string, number[][]][] = [
      ["Repeated stored serial values", report.serial_groups],
      ["Stored GPS points within 100m of first file", report.location_groups],
      ["Recorded dates without time zones", report.local_date_order.length > 1 ? [report.local_date_order] : []],
      ["Recorded dates with normalized time zones", report.utc_date_order.length > 1 ? [report.utc_date_order] : []],
    ];
    const relationshipGroups = blocks.flatMap(([, groups]) => groups);
    const relatedFiles = new Set(relationshipGroups.flat()).size;
    status.textContent = currentLocale() === "uk"
      ? `Перевірено ${report.total} / ${items.length} файлів · пов’язано файлів: ${relatedFiles} · груп зв’язків: ${relationshipGroups.length} · не перевірено: ${items.length - report.total} · GPS-точок: ${report.located_files}`
      : `${report.total} / ${items.length} files checked · ${relatedFiles} files in ${relationshipGroups.length} relationship groups · ${items.length - report.total} not checked · ${report.located_files} stored GPS points`;
    for (const [label, groups] of blocks) {
      dialog.append(el("h3", label));
      if (!groups.length) { dialog.append(el("p", "No comparable group found.")); continue; }
      const list = el("ul"); list.className = "photo-tool-results";
      for (const [groupIndex, group] of groups.entries()) {
        const row = el("li"); row.className = "photo-mosaic-group";
        const files = el("span"); files.className = "photo-mosaic-group-files";
        group.forEach((index, position) => {
          if (position) files.append(document.createTextNode(" → "));
          const item = items[index - 1];
          if (!item) {
            files.append(document.createTextNode(String(index)));
            return;
          }
          const filename = item.file.webkitRelativePath || item.file.name;
          const target = el("button", `${index}: ${filename}`);
          target.type = "button";
          target.className = "photo-mosaic-file";
          const action = translateText("Show this file in the batch list", currentLocale());
          const accessibleName = `${action}: ${index}: ${filename}`;
          target.setAttribute("aria-label", accessibleName);
          target.title = accessibleName;
          target.addEventListener("click", () => {
            dialog.addEventListener("close", () => showFileInBatch(index - 1), { once: true });
            dialog.close();
          });
          files.append(target);
        });
        row.append(files);
        const eligible = group.some((index) => {
          const item = items[index - 1];
          return item !== undefined && cleanable(item);
        });
        const select = el("button", eligible ? "Select these photos for clean copies" : "No supported clean copies in this group");
        select.type = "button";
        select.className = "btn photo-mosaic-select";
        select.disabled = !eligible;
        const groupName = currentLocale() === "uk" ? `група ${groupIndex + 1}` : `group ${groupIndex + 1}`;
        select.setAttribute(
          "aria-label",
          `${translateText(eligible ? "Select these photos for clean copies" : "No supported clean copies in this group", currentLocale())}: ${translateText(label, currentLocale())}, ${groupName}`,
        );
        if (eligible) {
          select.addEventListener("click", () => {
            dialog.addEventListener("close", () => {
              selectGroupForCleaning(group);
              showFileInBatch(group[0]! - 1);
            }, { once: true });
            dialog.close();
          });
        }
        row.append(select);
        list.append(row);
      }
      dialog.append(list);
    }
    const save = el("button", "Save mosaic report"); save.className = "btn btn-primary";
    const notChecked = items.flatMap((item, index) => item.photoSignal ? [] : [index + 1]);
    save.onclick = () => saveAs("hexscope-photo-mosaic.json", new Blob([JSON.stringify({ ...report, input_total: items.length, not_checked_indexes: notChecked }, null, 2)], { type: "application/json" }));
    dialog.append(
      el("p", "Every group refers to file indexes. The saved JSON contains no filenames, coordinates, dates or serial values."),
      save,
    );
  } catch (e) { if (dialog.open) status.textContent = e instanceof Error ? e.message : String(e); }
}
