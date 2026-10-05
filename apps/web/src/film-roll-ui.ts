import { call } from "./rpc";
import { StoredZip } from "./zipwrite";
import { saveAs } from "./files";
import { DEFAULT_FILM_SETTINGS, filmRecipe, readFilmRecipe, type FilmSettings } from "./film-lab";
import { registerLazyTranslations, currentLocale } from "./i18n";
import "./photo-tools.css";
registerLazyTranslations({
  "Film roll": "Плівковий рулон", "Close film roll": "Закрити плівковий рулон", "Choose scans": "Вибрати скани", "Choose scan folder": "Вибрати теку зі сканами",
  "Roll recipe": "Рецепт рулону", "Output format": "Формат копій", "Process roll": "Обробити рулон", "Download roll ZIP": "Завантажити ZIP рулону", "Load calibrated recipe": "Завантажити відкалібрований рецепт",
  "Process a whole roll with one recipe and a shared film base.": "Обробіть увесь рулон одним рецептом і спільною основою плівки.", "Files stay in this tab. Originals are preserved. Without a calibrated base, the first successful frame supplies the estimate for the roll.": "Файли залишаються у вкладці. Оригінали зберігаються. Без каліброваної основи перший успішний кадр задає оцінку для рулону.",
  "Scans": "Скани", "Scan type": "Тип скану", "JPEG · sharing · 8 bit": "JPEG · для надсилання · 8 біт", "TIFF · 16 bit": "TIFF · 16 біт", "Use a recipe from Film Photo Lab if you have one.": "За наявності використайте рецепт із лабораторії плівкового фото.", "Choose recipe": "Вибрати рецепт", "More settings and processing notes": "Додаткові налаштування й примітки до обробки", "Processing results": "Результати обробки", "Advanced recipe editor": "Розширений редактор рецепта",
  "First processed frame": "Перший оброблений кадр", "Invalid film recipe.": "Некоректний рецепт плівки.", "Invalid film settings.": "Некоректні налаштування плівки.", "Recipe must be smaller than 16 KiB.": "Рецепт має бути меншим за 16 КіБ.", "This scan exceeds the 50 MiB file limit.": "Цей скан перевищує ліміт у 50 МіБ.", "The source dimensions could not be inspected.": "Не вдалося визначити розміри джерела.", "This browser cannot process scan pixels.": "Цей браузер не може обробити пікселі скану.", "This browser cannot encode a copy.": "Цей браузер не може закодувати копію.", "JPEG encoding is unavailable.": "Кодування JPEG недоступне.", "Output dimensions differ.": "Розміри копії не збігаються.", "Processing unavailable.": "Обробка недоступна.",
  "This scan exceeds the film lab's 120 megapixel source limit.": "Скан перевищує ліміт джерела лабораторії у 120 мегапікселів.", "This browser cannot resize the scan safely.": "Цей браузер не може безпечно змінити розмір скану.", "This browser cannot encode a JPEG copy.": "Цей браузер не може закодувати копію JPEG.",
  "TIFF keeps 16-bit processing for 16-bit TIFF sources. Browser-decoded JPEG/PNG/WebP use 8-bit pixels. At most 12 MP per output; sharing JPEG also caps the side at 4096.": "TIFF зберігає 16-бітну обробку 16-бітних TIFF-джерел. JPEG/PNG/WebP, декодовані браузером, мають 8-бітні пікселі. Копія — до 12 МП; сторона JPEG — до 4096 пікселів.",
  "TIFF samples are assumed sRGB; embedded ICC profiles are reported, not applied. JPEG/PNG/WebP use the browser's color conversion to sRGB. These are adjustable renditions, without scanner-calibrated archival recovery. Metadata and visible content are not certified safe.": "Зразки TIFF вважаються sRGB; вбудовані ICC-профілі позначаються, але не застосовуються. JPEG/PNG/WebP використовують перетворення кольору браузером у sRGB. Це налаштовувані копії, без архівного відновлення за профілем сканера. Безпечність метаданих і видимого вмісту не засвідчується.",
  "Positive / already developed": "Позитив / уже проявлене зображення", "Color negative": "Кольоровий негатив", "Black and white negative": "Чорно-білий негатив",
});
const message = (en: string, uk: string) => currentLocale() === "uk" ? uk : en;
const translatedErrors: Record<string, string> = {
  "Invalid film recipe.": "Некоректний рецепт плівки.",
  "Invalid film settings.": "Некоректні налаштування плівки.",
  "Invalid film crop.": "Некоректне кадрування плівкового скану.",
  "Invalid film raster.": "Некоректні дані пікселів плівкового скану.",
  "Invalid film sample.": "Некоректний зразок плівки.",
  "Recipe must be smaller than 16 KiB.": "Рецепт має бути меншим за 16 КіБ.",
  "This scan exceeds the 50 MiB file limit.": "Цей скан перевищує ліміт у 50 МіБ.",
  "This scan exceeds the film lab's 50 MiB file limit.": "Файл скану перевищує ліміт лабораторії у 50 МіБ.",
  "This scan exceeds the film lab's 120 megapixel source limit.": "Скан перевищує ліміт джерела лабораторії у 120 мегапікселів.",
  "The source dimensions could not be inspected.": "Не вдалося визначити розміри джерела.",
  "This browser cannot process scan pixels.": "Цей браузер не може обробити пікселі скану.",
  "This browser cannot encode a copy.": "Цей браузер не може закодувати копію.",
  "This browser cannot encode a JPEG copy.": "Цей браузер не може закодувати копію JPEG.",
  "This browser cannot resize the scan safely.": "Цей браузер не може безпечно змінити розмір скану.",
  "JPEG encoding is unavailable.": "Кодування JPEG недоступне.",
  "Output dimensions differ.": "Розміри копії не збігаються.",
  "Processing unavailable.": "Обробка недоступна.",
  "hexscope was updated while this page was open. Reload the page to read it": "Hexscope оновився, поки ця сторінка була відкрита. Перезавантажте її, щоб продовжити.",
  "you are offline, and this part of hexscope has not been saved for offline use yet": "Ви офлайн, а цю частину Hexscope ще не збережено для роботи без мережі.",
  "it needs more memory than this browser tab has. Close other tabs and try again, or use the command line tool": "Цій вкладці бракує пам'яті. Закрийте інші вкладки або скористайтеся CLI.",
};
const errorMessage = (error: unknown): string => {
  const detail = error instanceof Error ? error.message : String(error);
  return currentLocale() === "uk" ? translatedErrors[detail] ?? detail : detail;
};
function el<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) { const e = document.createElement(tag); if (text) e.textContent = text; return e; }
export function showFilmRoll(initial: File[] = []): void {
  const dialog = el("dialog"); dialog.className = "photo-tool film-roll"; dialog.setAttribute("aria-labelledby", "film-roll-title");
  const title = el("h2", "Film roll"); title.id = "film-roll-title";
  const header = el("header"); header.className = "film-roll-header";
  const close = el("button", "Close"); close.type = "button"; close.className = "btn"; close.setAttribute("aria-label", "Close film roll");
  header.append(title, close);
  const body = el("div"); body.className = "film-roll-body";
  const intro = el("p", "Process a whole roll with one recipe and a shared film base."); intro.className = "film-roll-intro";
  const help = el("p", "Files stay in this tab. Originals are preserved. Without a calibrated base, the first successful frame supplies the estimate for the roll."); help.className = "film-roll-help";
  const sources = el("fieldset"); sources.className = "film-roll-sources"; sources.append(el("legend", "Scans"));
  const pickers = el("div"); pickers.className = "film-roll-picker-row";
  const picker = el("input"); picker.type = "file"; picker.multiple = true; picker.accept = ".jpg,.jpeg,.png,.webp,.tif,.tiff"; picker.setAttribute("aria-label", "Choose scans");
  const folder = el("input"); folder.type = "file"; folder.multiple = true; folder.accept = ".jpg,.jpeg,.png,.webp,.tif,.tiff"; folder.setAttribute("webkitdirectory", ""); folder.setAttribute("aria-label", "Choose scan folder");
  picker.className = folder.className = "film-roll-file-input";
  const chooseLabel = el("label"); chooseLabel.className = "film-roll-picker"; chooseLabel.append(el("span", "Choose scans"), picker);
  const folderLabel = el("label"); folderLabel.className = "film-roll-picker"; folderLabel.append(el("span", "Choose scan folder"), folder);
  pickers.append(chooseLabel, folderLabel); sources.append(pickers);
  const options = el("div"); options.className = "film-roll-options";
  const mode = el("select"); mode.setAttribute("aria-label", "Scan type");
  for (const [value, label] of [["positive", "Positive / already developed"], ["color-negative", "Color negative"], ["mono-negative", "Black and white negative"]]) { const o = el("option", label); o.value = value; mode.append(o); }
  const format = el("select"); format.setAttribute("aria-label", "Output format");
  for (const [value, label] of [["jpeg", "JPEG · sharing · 8 bit"], ["tiff16", "TIFF · 16 bit"]]) { const o = el("option", label); o.value = value; format.append(o); }
  const modeLabel = el("label"); modeLabel.className = "film-roll-field"; modeLabel.append(el("span", "Scan type"), mode);
  const formatLabel = el("label"); formatLabel.className = "film-roll-field"; formatLabel.append(el("span", "Output format"), format);
  options.append(modeLabel, formatLabel);
  const recipeLoadGroup = el("div"); recipeLoadGroup.className = "film-roll-recipe-load";
  const recipeCopy = el("div"); recipeCopy.className = "film-roll-recipe-copy"; recipeCopy.append(el("strong", "Load calibrated recipe"), el("p", "Use a recipe from Film Photo Lab if you have one."));
  const recipeLoad = el("input"); recipeLoad.type = "file"; recipeLoad.accept = ".json"; recipeLoad.setAttribute("aria-label", "Load calibrated recipe");
  recipeLoad.className = "film-roll-file-input";
  const recipeLabel = el("label"); recipeLabel.className = "film-roll-picker film-roll-picker-secondary"; recipeLabel.append(el("span", "Choose recipe"), recipeLoad);
  recipeLoadGroup.append(recipeCopy, recipeLabel);
  const advanced = el("details"); advanced.className = "film-roll-advanced"; advanced.append(el("summary", "More settings and processing notes"));
  const recipeHelp = el("p", "Advanced recipe editor"); recipeHelp.className = "film-roll-advanced-label";
  const recipe = el("textarea"); recipe.setAttribute("aria-label", "Roll recipe"); recipe.spellcheck = false; recipe.value = filmRecipe(DEFAULT_FILM_SETTINGS);
  const recipeEditor = el("div"); recipeEditor.className = "film-roll-recipe-editor"; recipeEditor.append(recipeHelp, recipe);
  const limits = el("p", "TIFF keeps 16-bit processing for 16-bit TIFF sources. Browser-decoded JPEG/PNG/WebP use 8-bit pixels. At most 12 MP per output; sharing JPEG also caps the side at 4096.");
  const color = el("p", "TIFF samples are assumed sRGB; embedded ICC profiles are reported, not applied. JPEG/PNG/WebP use the browser's color conversion to sRGB. These are adjustable renditions, without scanner-calibrated archival recovery. Metadata and visible content are not certified safe.");
  const notes = el("div"); notes.className = "film-roll-notes"; notes.append(limits, color);
  advanced.append(recipeEditor, notes);
  const process = el("button", "Process roll"); process.type = "button"; process.className = "btn btn-primary";
  const download = el("button", "Download roll ZIP"); download.className = "btn"; download.hidden = true;
  download.type = "button";
  const status = el("p"); status.className = "film-roll-status"; status.setAttribute("role", "status"); status.setAttribute("aria-live", "polite");
  const results = el("ol"); results.className = "photo-tool-results film-roll-results";
  const preview = el("img"); preview.alt = message("First processed frame", "Перший оброблений кадр"); preview.hidden = true;
  const output = el("section"); output.className = "film-roll-output"; output.setAttribute("aria-label", "Processing results");
  output.hidden = true;
  output.append(el("h3", "Processing results"), preview, results);
  body.append(intro, help, sources, options, recipeLoadGroup, advanced, output);
  const footer = el("footer"); footer.className = "film-roll-footer";
  const actions = el("div"); actions.className = "film-roll-actions"; actions.append(process, download);
  footer.append(status, actions);
  dialog.append(header, body, footer); document.body.append(dialog);
  let files = initial.filter((f) => /\.(jpe?g|png|webp|tiff?)$/i.test(f.name)); let generation = 0; let archive: Blob | null = null; let previewUrl = "";
  const selection = () => { status.textContent = message(`${files.length} scans selected · limit 1000`, `Вибрано ${files.length} сканів · межа 1000`); process.disabled = files.length === 0 || files.length > 1000; archive = null; download.hidden = true; };
  picker.onchange = () => { generation++; files = [...picker.files ?? []]; selection(); };
  folder.onchange = () => { generation++; files = [...folder.files ?? []].filter((f) => /\.(jpe?g|png|webp|tiff?)$/i.test(f.name)); selection(); };
  mode.onchange = () => { try { recipe.value = filmRecipe({ ...readFilmRecipe(recipe.value), mode: mode.value as FilmSettings["mode"] }); } catch { advanced.open = true; recipe.focus(); status.textContent = errorMessage(new Error("Invalid film recipe.")); } };
  recipeLoad.onchange = async () => {
    const file = recipeLoad.files?.[0], token = ++generation;
    try { if (!file || file.size > 16384) throw new Error("Recipe must be smaller than 16 KiB."); const text = await file.text(); const s = readFilmRecipe(text); if (token !== generation || !dialog.open) return; recipe.value = filmRecipe(s); mode.value = s.mode; } catch (e) { if (token === generation) status.textContent = errorMessage(e); }
  };
  process.onclick = async () => {
    const token = ++generation, list = files.slice(), outputFormat = format.value as "jpeg" | "tiff16";
    let settings: FilmSettings;
    try { settings = readFilmRecipe(recipe.value); } catch (e) { advanced.open = true; recipe.focus(); status.textContent = errorMessage(e); return; }
    if (!list.length || list.length > 1000) return;
    process.disabled = true; picker.disabled = folder.disabled = recipeLoad.disabled = mode.disabled = format.disabled = recipe.disabled = true; download.hidden = true; output.hidden = false; archive = null; results.replaceChildren();
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
          made++;
          row.textContent += message(
            ` · ${receipt.width} × ${receipt.height} · source ${receipt.sourceDepth} bit → output ${receipt.outputDepth} bit${receipt.limited ? " · resized" : ""}${receipt.profilePresent === null ? " · ICC not checked" : receipt.profilePresent ? (receipt.colorHandling === "browser_srgb_conversion" ? " · ICC detected · browser color conversion" : " · ICC detected · not applied") : ""}`,
            ` · ${receipt.width} × ${receipt.height} · джерело ${receipt.sourceDepth} біт → копія ${receipt.outputDepth} біт${receipt.limited ? " · розмір зменшено" : ""}${receipt.profilePresent === null ? " · ICC не перевірено" : receipt.profilePresent ? (receipt.colorHandling === "browser_srgb_conversion" ? " · ICC виявлено · браузерне перетворення кольору" : " · ICC виявлено · не застосовано") : ""}`,
          );
          rows.push({ index: index + 1, operation_state: "written", ...receipt });
          if (made === 1 && outputFormat === "jpeg") { if (previewUrl) URL.revokeObjectURL(previewUrl); previewUrl = URL.createObjectURL(copy); preview.src = previewUrl; preview.hidden = false; }
        } catch (e) { if (token !== generation || !dialog.open) return; row.textContent += message(` · Not created: ${errorMessage(e)}`, ` · Не створено: ${errorMessage(e)}`); rows.push({ index: index + 1, operation_state: "not_created", check: "unavailable" }); }
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
