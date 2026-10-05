import { call } from "./rpc";
import { StoredZip } from "./zipwrite";
import { saveAs } from "./files";
import { DEFAULT_FILM_SETTINGS, filmRecipe, readFilmRecipe, type FilmSettings } from "./film-lab";
import { registerLazyTranslations, currentLocale } from "./i18n";
import "./photo-tools.css";
registerLazyTranslations({
  "Film roll": "Плівковий рулон", "Close film roll": "Закрити плівковий рулон", "Choose scans": "Вибрати скани", "Choose scan folder": "Вибрати теку зі сканами",
  "Roll recipe": "Рецепт рулону", "Output format": "Формат копій", "Process roll": "Обробити рулон", "Download roll ZIP": "Завантажити ZIP рулону", "Load calibrated recipe": "Завантажити відкалібрований рецепт",
  "One recipe, one film base, all frames. Files stay in this tab; originals are preserved.": "Один рецепт і одна основа плівки для всіх кадрів. Файли залишаються у вкладці; оригінали зберігаються.",
  "Load a recipe saved in the single-frame Film Photo Lab. Without a calibrated base, the first successful frame supplies an estimate reused for the roll.": "Завантажте рецепт із лабораторії окремого кадру. Якщо основу не відкалібровано, оцінка з першого успішного кадру застосовується до рулону.",
  "TIFF keeps 16-bit processing for 16-bit TIFF sources. Browser-decoded JPEG/PNG/WebP use 8-bit pixels. At most 12 MP per output; sharing JPEG also caps the side at 4096.": "TIFF зберігає 16-бітну обробку 16-бітних TIFF-джерел. JPEG/PNG/WebP, декодовані браузером, мають 8-бітні пікселі. Копія — до 12 МП; сторона JPEG — до 4096 пікселів.",
  "Samples are assumed sRGB. Embedded ICC profiles are reported, not applied. These are adjustable renditions, not scanner-calibrated archival recovery. Metadata and visible content are not certified safe.": "Зразки вважаються sRGB. Вбудовані ICC-профілі позначаються, але не застосовуються. Це налаштовувані копії, без архівного відновлення за профілем сканера. Безпечність метаданих і видимого вмісту не засвідчується.",
  "Positive / already developed": "Позитив / уже проявлене зображення", "Color negative": "Кольоровий негатив", "Black and white negative": "Чорно-білий негатив",
});
const message = (en: string, uk: string) => currentLocale() === "uk" ? uk : en;
function el<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) { const e = document.createElement(tag); if (text) e.textContent = text; return e; }
export function showFilmRoll(initial: File[] = []): void {
  const dialog = el("dialog"); dialog.className = "photo-tool film-roll"; dialog.setAttribute("aria-labelledby", "film-roll-title");
  const title = el("h2", "Film roll"); title.id = "film-roll-title";
  const close = el("button", "Close"); close.className = "btn"; close.setAttribute("aria-label", "Close film roll");
  const intro = el("p", "One recipe, one film base, all frames. Files stay in this tab; originals are preserved.");
  const help = el("p", "Load a recipe saved in the single-frame Film Photo Lab. Without a calibrated base, the first successful frame supplies an estimate reused for the roll.");
  const controls = el("div"); controls.className = "photo-tool-controls";
  const picker = el("input"); picker.type = "file"; picker.multiple = true; picker.accept = ".jpg,.jpeg,.png,.webp,.tif,.tiff"; picker.setAttribute("aria-label", "Choose scans");
  const folder = el("input"); folder.type = "file"; folder.multiple = true; folder.setAttribute("webkitdirectory", ""); folder.setAttribute("aria-label", "Choose scan folder");
  const chooseLabel = el("label", "Choose scans"); chooseLabel.append(picker); const folderLabel = el("label", "Choose scan folder"); folderLabel.append(folder);
  const mode = el("select"); mode.setAttribute("aria-label", "Scan type");
  for (const [value, label] of [["positive", "Positive / already developed"], ["color-negative", "Color negative"], ["mono-negative", "Black and white negative"]]) { const o = el("option", label); o.value = value; mode.append(o); }
  const format = el("select"); format.setAttribute("aria-label", "Output format");
  for (const [value, label] of [["jpeg", "JPEG · sharing · 8 bit"], ["tiff16", "TIFF · 16 bit"]]) { const o = el("option", label); o.value = value; format.append(o); }
  const recipeLoad = el("input"); recipeLoad.type = "file"; recipeLoad.accept = ".json"; recipeLoad.setAttribute("aria-label", "Load calibrated recipe");
  const recipeLabel = el("label", "Load calibrated recipe"); recipeLabel.append(recipeLoad);
  const advanced = el("details"); advanced.append(el("summary", "Roll recipe"));
  const recipe = el("textarea"); recipe.setAttribute("aria-label", "Roll recipe"); recipe.spellcheck = false; recipe.value = filmRecipe(DEFAULT_FILM_SETTINGS); advanced.append(recipe);
  const limits = el("p", "TIFF keeps 16-bit processing for 16-bit TIFF sources. Browser-decoded JPEG/PNG/WebP use 8-bit pixels. At most 12 MP per output; sharing JPEG also caps the side at 4096.");
  const color = el("p", "Samples are assumed sRGB. Embedded ICC profiles are reported, not applied. These are adjustable renditions, not scanner-calibrated archival recovery. Metadata and visible content are not certified safe.");
  const process = el("button", "Process roll"); process.className = "btn btn-primary";
  const download = el("button", "Download roll ZIP"); download.className = "btn"; download.hidden = true;
  const status = el("p"); status.setAttribute("role", "status"); status.setAttribute("aria-live", "polite");
  const results = el("ol"); results.className = "photo-tool-results";
  const preview = el("img"); preview.alt = "First processed frame"; preview.hidden = true;
  controls.append(chooseLabel, folderLabel, mode, format, recipeLabel, advanced, process, download);
  dialog.append(close, title, intro, help, controls, limits, color, status, preview, results); document.body.append(dialog);
  let files = initial.filter((f) => /\.(jpe?g|png|webp|tiff?)$/i.test(f.name)); let generation = 0; let archive: Blob | null = null; let previewUrl = "";
  const selection = () => { status.textContent = message(`${files.length} scans selected · limit 1000`, `Вибрано ${files.length} сканів · межа 1000`); process.disabled = files.length === 0 || files.length > 1000; archive = null; download.hidden = true; };
  picker.onchange = () => { generation++; files = [...picker.files ?? []]; selection(); };
  folder.onchange = () => { generation++; files = [...folder.files ?? []].filter((f) => /\.(jpe?g|png|webp|tiff?)$/i.test(f.name)); selection(); };
  mode.onchange = () => { try { recipe.value = filmRecipe({ ...readFilmRecipe(recipe.value), mode: mode.value as FilmSettings["mode"] }); } catch { status.textContent = "Invalid film recipe."; } };
  recipeLoad.onchange = async () => {
    const file = recipeLoad.files?.[0], token = ++generation;
    try { if (!file || file.size > 16384) throw new Error("Recipe must be smaller than 16 KiB."); const text = await file.text(); const s = readFilmRecipe(text); if (token !== generation || !dialog.open) return; recipe.value = filmRecipe(s); mode.value = s.mode; } catch (e) { if (token === generation) status.textContent = String(e); }
  };
  process.onclick = async () => {
    const token = ++generation, list = files.slice(), outputFormat = format.value as "jpeg" | "tiff16";
    let settings: FilmSettings;
    try { settings = readFilmRecipe(recipe.value); } catch (e) { status.textContent = String(e); return; }
    if (!list.length || list.length > 1000) return;
    process.disabled = true; picker.disabled = folder.disabled = recipeLoad.disabled = mode.disabled = format.disabled = recipe.disabled = true; download.hidden = true; archive = null; results.replaceChildren();
    const zip = new StoredZip(); const rows: Record<string, unknown>[] = []; let made = 0;
    try {
      for (let index = 0; index < list.length; index++) {
        status.textContent = message(`Processing ${index + 1} / ${list.length}…`, `Обробляю ${index + 1} / ${list.length}…`);
        const row = el("li", `${index + 1}. ${list[index].name}`); results.append(row);
        try {
          const response = await call({ type: "filmRoll", source: list[index], settings, format: outputFormat });
          if (token !== generation || !dialog.open) return;
          if (response.type !== "filmRollOutput") throw new Error(response.type === "error" ? response.message : "Processing unavailable.");
          const { copy, ...receipt } = response.result;
          if (!settings.baseColor) { settings = { ...settings, baseColor: receipt.baseColor }; recipe.value = filmRecipe(settings); }
          await zip.add(`${String(index + 1).padStart(6, "0")}-film.${outputFormat === "jpeg" ? "jpg" : "tif"}`, copy);
          if (token !== generation || !dialog.open) return;
          made++; row.textContent += ` · ${receipt.width} × ${receipt.height} · source ${receipt.sourceDepth} bit → output ${receipt.outputDepth} bit${receipt.limited ? " · resized" : ""}${receipt.profilePresent ? " · ICC not applied" : ""}`;
          rows.push({ index: index + 1, operation_state: "written", ...receipt });
          if (made === 1 && outputFormat === "jpeg") { if (previewUrl) URL.revokeObjectURL(previewUrl); previewUrl = URL.createObjectURL(copy); preview.src = previewUrl; preview.hidden = false; }
        } catch (e) { if (token !== generation || !dialog.open) return; row.textContent += ` · Not created: ${e instanceof Error ? e.message : String(e)}`; rows.push({ index: index + 1, operation_state: "not_created", check: "unavailable" }); }
      }
      if (token !== generation || !dialog.open) return;
      await zip.add("hexscope-film-report.json", new Blob([JSON.stringify({ schema: "hexscope.film-roll", version: 1, algorithm: "srgb-density-v1", color_assumption: "srgb_encoded_samples", total: list.length, written: made, failed: list.length - made, files: rows }, null, 2)]));
      if (token !== generation || !dialog.open) return;
      archive = zip.finish(); download.hidden = false; status.textContent = message(`${made} / ${list.length} copies ready · ${list.length - made} not created. Originals preserved. Metadata and visible content: not certified safe.`, `Готово ${made} / ${list.length} копій · ${list.length - made} не створено. Оригінали збережено. Безпечність метаданих і видимого вмісту не засвідчено.`);
    } finally { if (token === generation) { picker.disabled = folder.disabled = recipeLoad.disabled = mode.disabled = format.disabled = recipe.disabled = false; process.disabled = false; } }
  };
  download.onclick = () => { if (archive) saveAs("hexscope-film-roll.zip", archive); };
  close.onclick = () => dialog.close();
  dialog.addEventListener("close", () => { generation++; if (previewUrl) URL.revokeObjectURL(previewUrl); archive = null; files = []; dialog.remove(); });
  selection(); dialog.showModal();
}
