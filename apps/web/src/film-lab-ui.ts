import { el } from "./dom";
import { saveAs } from "./files";
import { call } from "./rpc";
import { registerLazyTranslations } from "./i18n";
import { DEFAULT_FILM_SETTINGS, filmRecipe, readFilmRecipe, type FilmSettings, type RGB } from "./film-lab";
import type { FilmScanReport } from "./filmscan";
import type { FileModel } from "./model";
import type { FilmRenderResult } from "./film-render";
import "./film-lab.css";

registerLazyTranslations({
  "Film photo lab": "Лабораторія плівкового фото",
  "Prepare a new sharing copy locally. Preview the crop and color before downloading.": "Підготуйте нову копію для надсилання локально. Перегляньте кадрування й колір перед завантаженням.",
  "Scan type": "Тип скану", "Positive / already developed": "Позитив / уже проявлене зображення",
  "Color negative": "Кольоровий негатив", "Black and white negative": "Чорно-білий негатив",
  "Crop visible borders": "Обрізати видимі краї", "Trim left (%)": "Обрізати зліва (%)", "Trim top (%)": "Обрізати зверху (%)",
  "Trim right (%)": "Обрізати справа (%)", "Trim bottom (%)": "Обрізати знизу (%)", "Use detected frame": "Використати знайдену рамку",
  "Review the suggested crop: image details and edge lettering inside the green rectangle will remain.": "Перевірте запропоноване кадрування: деталі зображення й написи всередині зеленої рамки залишаться.",
  "Original scan": "Оригінальний скан", "Sharing preview": "Перегляд копії для надсилання",
  "Amber bands mark repeated edge patterns. The green rectangle is the retained crop.": "Помаранчеві смуги позначають повторювані ознаки краю. Зелена рамка показує частину, яка залишиться.",
  "Film base calibration": "Калібрування основи плівки", "Film base red": "Основа плівки: червоний", "Film base green": "Основа плівки: зелений",
  "Film base blue": "Основа плівки: синій", "Pick unexposed film": "Вибрати неекспоновану плівку", "Click a clear film border in the original scan.": "Натисніть чисту ділянку краю плівки на оригінальному скані.",
  "Sample by coordinates": "Вибрати зразок за координатами", "Sample x (%)": "Координата зразка x (%)", "Sample y (%)": "Координата зразка y (%)", "Sample film base": "Взяти зразок основи плівки",
  "Estimate base automatically": "Оцінити основу автоматично",
  "For color negatives, sample unexposed film or enter its RGB values. The automatic base is an estimate. Color from an 8-bit scan is approximate.": "Для кольорового негативу виберіть неекспоновану ділянку плівки або введіть її RGB. Автоматична основа — лише оцінка. Колір із 8-бітного скану приблизний.",
  "Exposure (EV)": "Експозиція (EV)", "Contrast": "Контраст", "Updating preview…": "Оновлення перегляду…", "Preview ready": "Перегляд готовий",
  "Creating and checking JPEG…": "Створення та перевірка JPEG…", "Create JPEG copy": "Створити JPEG-копію", "Download JPEG": "Завантажити JPEG",
  "Save recipe": "Зберегти рецепт", "Load recipe": "Завантажити рецепт", "Reset adjustments": "Скинути налаштування",
  "Reuse these settings on another scan. A recipe contains adjustments, not the photo or its filename.": "Застосуйте налаштування до іншого скану. Рецепт містить параметри, без фото чи назви файла.",
  "Could not load this recipe. Choose a Hexscope film recipe, version 1, smaller than 16 KiB.": "Не вдалося прочитати рецепт. Виберіть рецепт плівкового фото Hexscope версії 1, менший за 16 КіБ.",
  "Could not process this scan. Try a smaller JPEG, PNG or WebP.": "Не вдалося обробити скан. Спробуйте менший JPEG, PNG або WebP.",
  "Preview: up to 1200 px. JPEG copy: up to 4096 px and 12 megapixels. Sources: up to 50 MiB and 120 megapixels. The master stays intact.": "Перегляд: до 1200 px. JPEG-копія: до 4096 px і 12 мегапікселів. Оригінали: до 50 МіБ і 120 мегапікселів. Майстер-копія зберігається.",
  "Output re-read": "Повторне читання результату", "No metadata findings detected in the output JPEG.": "У створеному JPEG не виявлено метаданих.",
  "Metadata findings still detected": "Ще виявлені метадані", "No QR codes detected in the output scan.": "У скані результату не виявлено QR-кодів.",
  "QR codes still detected": "Ще виявлені QR-коди", "QR check unavailable.": "Перевірка QR недоступна.",
  "Repeated edge patterns still detected.": "Повторювані ознаки краю ще виявляються.", "No repeated edge patterns detected in the output scan.": "У скані результату не виявлено повторюваних ознак краю.",
  "Output check unavailable; the copy is still downloadable.": "Перевірка результату недоступна; копію все одно можна завантажити.",
  "Review the image: visible names, edge lettering and recognizable content may remain. A pixel crop is separate from metadata removal.": "Перегляньте зображення: видимі імена, написи на плівці й упізнаваний вміст можуть залишитися. Кадрування пікселів — окрема дія від видалення метаданих.",
  "Source pixels with clipped dark channels": "Пікселі оригіналу з відсіченими темними каналами", "Source pixels with clipped bright channels": "Пікселі оригіналу з відсіченими світлими каналами",
  "Clipped channel detail cannot be recovered by these adjustments.": "Ці налаштування не відновлюють деталі у відсічених каналах.",
  "Copy SHA-256": "SHA-256 копії", "Copy resized for the sharing limit.": "Копію зменшено відповідно до ліміту для надсилання.",
});

/** One mounted lab owns its URLs, coalesces previews and ignores stale responses. */
export function filmLab(model: FileModel, report: FilmScanReport): { view: HTMLElement; dispose(): void } {
  const view = el("section", "film-lab");
  view.setAttribute("aria-label", "Film photo lab");
  view.dataset.state = "loading";
  view.append(el("p", "hint", "Prepare a new sharing copy locally. Preview the crop and color before downloading."));
  const source = model.source ?? new Blob([model.bytes as BlobPart]);
  let settings: FilmSettings = { ...DEFAULT_FILM_SETTINGS, crop: [0, 0, 0, 0] };
  let disposed = false, busy = false, dirty = false, exporting = false, revision = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pendingPoint: [number, number] | undefined;
  let sampling = false;
  let calibrationRevision = 0;
  let prepared: FilmRenderResult | null = null;
  const urls = new Map<HTMLImageElement, string>();
  const message = el("p", "hint film-lab-message", "Updating preview…");
  message.setAttribute("role", "status");
  const controls = el("div", "film-lab-controls");
  const labeled = <K extends keyof HTMLElementTagNameMap>(name: string, input: HTMLElementTagNameMap[K]) => {
    const label = el("label", "film-lab-field");
    label.append(el("span", undefined, name), input);
    return label;
  };
  const mode = el("select");
  for (const [value, label] of [["positive", "Positive / already developed"], ["color-negative", "Color negative"], ["mono-negative", "Black and white negative"]]) {
    const option = el("option", undefined, label); option.value = value; mode.append(option);
  }
  controls.append(labeled("Scan type", mode));
  const cropFields = el("fieldset", "film-lab-crop");
  cropFields.append(el("legend", undefined, "Crop visible borders"));
  const cropInputs = ["Trim left (%)", "Trim top (%)", "Trim right (%)", "Trim bottom (%)"].map((name, i) => {
    const input = el("input"); input.type = "number"; input.min = "0"; input.max = "45"; input.step = "0.1"; input.value = "0";
    input.addEventListener("input", () => { settings.crop[i] = Math.max(0, Math.min(45, input.valueAsNumber || 0)) / 100; schedule(); });
    cropFields.append(labeled(name, input));
    return input;
  });
  if (report.frameBounds) {
    const suggested = el("button", "btn", "Use detected frame");
    suggested.type = "button";
    suggested.addEventListener("click", () => {
      const [l, t, r, b] = report.frameBounds!;
      settings.crop = [l, t, 1 - r, 1 - b].map((v) => Math.max(0, Math.min(0.45, v))) as FilmSettings["crop"];
      syncControls(); schedule();
    });
    cropFields.append(suggested);
  }
  controls.append(cropFields, el("p", "hint", "Review the suggested crop: image details and edge lettering inside the green rectangle will remain."));

  const images = el("div", "film-lab-images");
  const original = el("img", "film-lab-source"); original.alt = "Original scan";
  const preview = el("img", "film-lab-preview"); preview.alt = "Sharing preview";
  const stage = el("div", "film-lab-original");
  stage.append(original);
  for (const edge of report.perforationEdges) {
    const mark = el("div", `film-edge-marker edge-${edge}`); mark.setAttribute("aria-hidden", "true"); stage.append(mark);
  }
  const cropBox = el("div", "film-crop-box"); cropBox.setAttribute("aria-hidden", "true"); stage.append(cropBox);
  for (const [content, caption] of [[stage, "Original scan"], [preview, "Sharing preview"]] as const) {
    const figure = el("figure"); figure.append(content, el("figcaption", undefined, caption)); images.append(figure);
  }
  const size = el("output", "mono film-lab-size");
  view.append(images, size, el("p", "hint", "Amber bands mark repeated edge patterns. The green rectangle is the retained crop."));

  const calibration = el("fieldset", "film-lab-base");
  calibration.append(el("legend", undefined, "Film base calibration"));
  const baseInputs = ["Film base red", "Film base green", "Film base blue"].map((name, i) => {
    const input = el("input"); input.type = "number"; input.min = "1"; input.max = "255"; input.step = "1"; input.value = "255";
    input.addEventListener("input", () => {
      const base = (settings.baseColor ?? [255, 255, 255]).slice() as RGB;
      calibrationRevision++; pendingPoint = undefined; stopSampling();
      base[i] = Math.max(1, Math.min(255, Math.round(input.valueAsNumber || 1))); settings.baseColor = base; schedule();
    });
    calibration.append(labeled(name, input)); return input;
  });
  const pick = el("button", "btn", "Pick unexposed film"); pick.type = "button"; pick.setAttribute("aria-pressed", "false");
  pick.addEventListener("click", () => {
    sampling = !sampling; pick.setAttribute("aria-pressed", String(sampling)); stage.classList.toggle("is-sampling", sampling);
    if (sampling) message.textContent = "Click a clear film border in the original scan.";
  });
  original.addEventListener("click", (event) => {
    if (!sampling || exporting) return;
    const rect = original.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    calibrationRevision++; pendingPoint = [(event.clientX - rect.left) / rect.width, (event.clientY - rect.top) / rect.height];
    sampling = false; pick.setAttribute("aria-pressed", "false"); stage.classList.remove("is-sampling"); schedule();
  });
  const automatic = el("button", "btn", "Estimate base automatically"); automatic.type = "button";
  automatic.addEventListener("click", () => { calibrationRevision++; pendingPoint = undefined; stopSampling(); settings.baseColor = null; schedule(); });
  calibration.append(pick, automatic, el("p", "hint", "For color negatives, sample unexposed film or enter its RGB values. The automatic base is an estimate. Color from an 8-bit scan is approximate."));
  const coordinates = el("details", "film-lab-coordinates");
  coordinates.append(el("summary", undefined, "Sample by coordinates"));
  const pointInputs = ["Sample x (%)", "Sample y (%)"].map((name) => {
    const input = el("input"); input.type = "number"; input.min = "0"; input.max = "100"; input.step = "0.1"; input.value = "5";
    coordinates.append(labeled(name, input)); return input;
  });
  const sampleAt = el("button", "btn", "Sample film base"); sampleAt.type = "button";
  sampleAt.addEventListener("click", () => {
    calibrationRevision++; stopSampling();
    pendingPoint = pointInputs.map((input) => Math.max(0, Math.min(100, input.valueAsNumber || 0)) / 100) as [number, number];
    schedule();
  });
  coordinates.append(sampleAt); calibration.append(coordinates);
  controls.append(calibration);
  const tonal = ([ ["Exposure (EV)", "exposure", -2, 2, 0.1], ["Contrast", "contrast", 0.5, 2, 0.05] ] as const).map(([name, key, min, max, step]) => {
    const input = el("input"); input.type = "range"; input.min = String(min); input.max = String(max); input.step = String(step);
    const output = el("output", "mono");
    const label = labeled(name, input); label.append(output);
    input.addEventListener("input", () => { settings[key] = input.valueAsNumber; output.textContent = input.value; schedule(); });
    controls.append(label); return { input, output, key };
  });
  mode.addEventListener("change", () => { calibrationRevision++; pendingPoint = undefined; sampling = false; pick.setAttribute("aria-pressed", "false"); stage.classList.remove("is-sampling"); settings.mode = mode.value as FilmSettings["mode"]; syncControls(); schedule(); });
  view.append(controls);
  const clipping = el("p", "hint film-lab-clipping");
  const check = el("section", "film-lab-check"); check.setAttribute("role", "status");
  const create = el("button", "btn btn-primary film-lab-create", "Create JPEG copy"); create.disabled = true;
  const download = el("button", "btn btn-primary film-lab-download", "Download JPEG"); download.hidden = true;
  download.addEventListener("click", () => { if (prepared) saveAs("film-copy.jpg", prepared.copy); });
  const save = el("button", "btn", "Save recipe"); save.disabled = true;
  save.addEventListener("click", () => saveAs("film-recipe.json", new Blob([filmRecipe(settings)], { type: "application/json" })));
  const recipeInput = el("input", "film-recipe-input"); recipeInput.type = "file"; recipeInput.accept = ".json,application/json"; recipeInput.hidden = true;
  const load = el("button", "btn", "Load recipe"); load.addEventListener("click", () => recipeInput.click());
  recipeInput.addEventListener("change", async () => {
    const file = recipeInput.files?.[0]; if (!file || exporting) return;
    const before = revision;
    try {
      if (file.size > 16_384) throw new Error("recipe too large");
      const next = readFilmRecipe(await file.text());
      if (disposed || before !== revision) return;
      calibrationRevision++; settings = next; pendingPoint = undefined; syncControls(); schedule();
    } catch { message.textContent = "Could not load this recipe. Choose a Hexscope film recipe, version 1, smaller than 16 KiB."; }
    finally { recipeInput.value = ""; }
  });
  const reset = el("button", "btn", "Reset adjustments");
  reset.addEventListener("click", () => { calibrationRevision++; settings = { ...DEFAULT_FILM_SETTINGS, crop: [0, 0, 0, 0] }; pendingPoint = undefined; syncControls(); schedule(); });
  const actions = el("div", "film-lab-actions"); actions.append(create, download, save, load, reset, recipeInput);
  view.append(clipping, message, actions, check,
    el("p", "hint", "Reuse these settings on another scan. A recipe contains adjustments, not the photo or its filename."),
    el("p", "hint", "Preview: up to 1200 px. JPEG copy: up to 4096 px and 12 megapixels. Sources: up to 50 MiB and 120 megapixels. The master stays intact."));

  function stopSampling(): void {
    sampling = false; pick.setAttribute("aria-pressed", "false"); stage.classList.remove("is-sampling");
  }
  function syncControls(): void {
    stopSampling();
    mode.value = settings.mode;
    calibration.hidden = settings.mode === "positive";
    cropInputs.forEach((input, i) => { input.value = String(Math.round(settings.crop[i] * 1000) / 10); });
    baseInputs.forEach((input, i) => { input.value = String(settings.baseColor?.[i] ?? 255); });
    tonal.forEach(({ input, output, key }) => { input.value = String(settings[key]); output.textContent = input.value; });
  }
  function putImage(image: HTMLImageElement, blob: Blob): void {
    const old = urls.get(image); if (old) URL.revokeObjectURL(old);
    const url = URL.createObjectURL(blob); urls.set(image, url); image.src = url;
  }
  function showResult(result: FilmRenderResult): void {
    putImage(preview, result.copy);
    if (result.sourcePreview) putImage(original, result.sourcePreview);
    size.textContent = `${result.dimensions[0]} × ${result.dimensions[1]}`;
    settings.baseColor = result.baseColor;
    baseInputs.forEach((input, i) => { input.value = String(result.baseColor[i]); });
    clipping.replaceChildren(el("span", undefined, "Source pixels with clipped dark channels"), document.createTextNode(`: ${result.clipping.shadows}% · `), el("span", undefined, "Source pixels with clipped bright channels"), document.createTextNode(`: ${result.clipping.highlights}%`));
    if (result.clipping.shadows || result.clipping.highlights) clipping.append(el("span", undefined, "Clipped channel detail cannot be recovered by these adjustments."));
  }
  function schedule(): void {
    if (disposed || exporting) return;
    revision++; dirty = true; prepared = null; download.hidden = true; check.replaceChildren(); create.disabled = save.disabled = true;
    view.dataset.state = "loading"; message.textContent = "Updating preview…";
    cropBox.style.inset = `${settings.crop[1] * 100}% ${settings.crop[2] * 100}% ${settings.crop[3] * 100}% ${settings.crop[0] * 100}%`;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void update(), 160);
  }
  async function update(): Promise<void> {
    if (busy || disposed || exporting) return;
    busy = true;
    try {
      while (dirty && !disposed) {
        dirty = false;
        const version = revision, calibrationVersion = calibrationRevision, point = pendingPoint; pendingPoint = undefined;
        const reply = await call({ type: "filmRender", source, dimensions: model.file.dimensions!, orientation: model.file.orientation, settings: { ...settings, crop: [...settings.crop] }, purpose: "preview", ...(point ? { point } : {}) });
        if (disposed) return;
        if (version !== revision) { if (point && calibrationVersion === calibrationRevision && !pendingPoint) pendingPoint = point; continue; }
        if (reply.type !== "filmRendered") { view.dataset.state = "error"; message.textContent = "Could not process this scan. Try a smaller JPEG, PNG or WebP."; continue; }
        showResult(reply.result); view.dataset.state = "ready"; message.textContent = "Preview ready"; create.disabled = save.disabled = false;
      }
    } finally { busy = false; }
  }
  create.addEventListener("click", async () => {
    if (busy || dirty || disposed || exporting) return;
    exporting = true; controls.inert = true; create.disabled = save.disabled = load.disabled = reset.disabled = true;
    view.dataset.state = "exporting"; message.textContent = "Creating and checking JPEG…";
    const version = revision;
    const reply = await call({ type: "filmRender", source, dimensions: model.file.dimensions!, orientation: model.file.orientation, settings, purpose: "export" });
    if (disposed || version !== revision) return;
    exporting = false; controls.inert = false; create.disabled = save.disabled = load.disabled = reset.disabled = false;
    if (reply.type !== "filmRendered") { view.dataset.state = "error"; message.textContent = "Could not process this scan. Try a smaller JPEG, PNG or WebP."; return; }
    prepared = reply.result; showResult(prepared); view.dataset.state = "ready"; message.textContent = "Preview ready"; download.hidden = false;
    check.replaceChildren(el("h3", undefined, "Output re-read"));
    const receipt = prepared.check;
    if (receipt?.status === "checked") {
      check.append(el("p", undefined, receipt.metadata.length ? "Metadata findings still detected" : "No metadata findings detected in the output JPEG."));
      if (receipt.metadata.length) { const list = el("ul"); receipt.metadata.forEach((label) => list.append(el("li", undefined, label))); check.append(list); }
      check.append(el("p", undefined, receipt.qrCount === null ? "QR check unavailable." : receipt.qrCount === 0 ? "No QR codes detected in the output scan." : "QR codes still detected"));
      if (receipt.qrCount) check.append(el("output", undefined, String(receipt.qrCount)));
      if (receipt.scan?.availability === "inspected") check.append(el("p", undefined, receipt.scan.evidenceStrength === "none" ? "No repeated edge patterns detected in the output scan." : "Repeated edge patterns still detected."));
      if (receipt.sha256) { const hash = el("details", "film-lab-hash"); hash.append(el("summary", undefined, "Copy SHA-256"), el("code", undefined, receipt.sha256)); check.append(hash); }
    } else check.append(el("p", "hint", "Output check unavailable; the copy is still downloadable."));
    if (prepared.limited) check.append(el("p", "hint", "Copy resized for the sharing limit."));
    check.append(el("p", "hint", "Review the image: visible names, edge lettering and recognizable content may remain. A pixel crop is separate from metadata removal."));
  });

  function dispose(): void {
    if (disposed) return;
    disposed = true; revision++; if (timer) clearTimeout(timer);
    for (const url of urls.values()) URL.revokeObjectURL(url);
    urls.clear(); prepared = null; observer.disconnect();
  }
  const observer = new MutationObserver(() => { if (!view.isConnected) dispose(); });
  observer.observe(document.body, { childList: true, subtree: true });
  syncControls(); schedule();
  return { view, dispose };
}
