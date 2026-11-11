import type { PageArea } from "./model";
import type { Shown } from "./pdf-pictures";
import { el } from "./dom";
import { currentLocale } from "./i18n";
import { recognizeLocally } from "./local-ocr";

/** Conservative AABB of all four corners in the PDF image's actual matrix. */
export function imageBoxOnPage(box: [number, number, number, number], matrix: number[]): PageArea {
  if (matrix.length !== 6 || !matrix.every(Number.isFinite)) throw new Error("Unreadable image placement");
  const [a, b, c, d, e, f] = matrix;
  const points = [box[0], box[2]].flatMap((x) => [1 - box[1], 1 - box[3]].map((y) => [a * x + c * y + e, b * x + d * y + f]));
  return [Math.min(...points.map((p) => p[0])), Math.min(...points.map((p) => p[1])), Math.max(...points.map((p) => p[0])), Math.max(...points.map((p) => p[1]))];
}

export function pdfOcrTools(pictures: Promise<Shown[]>, onBox: (area: PageArea) => void, onPreview: (area: PageArea) => void): HTMLElement {
  const say = (en: string, uk: string) => currentLocale() === "uk" ? uk : en;
  const host = el("div", "pdf-ocr-tools");
  host.append(el("p", "hint", say(
    "Choose an extracted picture and check each suggested area on the page before adding it. OCR may miss text.",
    "Виберіть витягнуте зображення й перевірте кожну запропоновану область на сторінці перед додаванням. OCR може пропустити текст.",
  )));
  const choice = el("select");
  choice.setAttribute("aria-label", say("Page image", "Зображення сторінки"));
  choice.disabled = true;
  const previewImage = el("button", "btn", say("Show selected image on page", "Показати вибране зображення на сторінці"));
  previewImage.disabled = true;
  const lang = el("select"); lang.setAttribute("aria-label", say("OCR language", "Мова OCR"));
  for (const [value, label] of [["eng", "English"], ["ukr", "Українська"]]) { const option = el("option", undefined, label); option.value = value; lang.append(option); }
  const run = el("button", "btn", say("Recognize the selected image locally", "Розпізнати вибране зображення локально"));
  const cancel = el("button", "btn", say("Cancel recognition", "Скасувати розпізнавання")); cancel.hidden = true;
  const status = el("p", "hint"); status.setAttribute("role", "status");
  const results = el("div");
  let controller: AbortController;
  let available: Shown[] = [];
  run.disabled = true;
  status.textContent = say("Loading page images…", "Завантажуємо зображення сторінки…");
  void pictures.then((images) => {
    available = images;
    if (images.length === 0) {
      status.textContent = say("No extractable page image", "Немає доступного зображення сторінки");
      return;
    }
    images.forEach((_, index) => {
      const option = el("option", undefined, `${say("Image", "Зображення")} ${index + 1}`);
      option.value = String(index);
      choice.append(option);
    });
    choice.disabled = run.disabled = previewImage.disabled = false;
    status.textContent = "";
  }).catch(() => {
    status.textContent = say("Could not read page images", "Не вдалося прочитати зображення сторінки");
  });
  choice.onchange = () => {
    results.replaceChildren();
    status.textContent = "";
  };
  previewImage.onclick = () => {
    const picture = available[Number(choice.value)];
    if (picture) onPreview(imageBoxOnPage([0, 0, 1, 1], picture.matrix));
  };
  cancel.onclick = () => controller?.abort();
  run.onclick = async () => {
    run.disabled = choice.disabled = lang.disabled = true; cancel.hidden = false; controller = new AbortController();
    results.replaceChildren(); status.textContent = say("Recognizing text locally…", "Локальне розпізнавання тексту…");
    // Removing this preview (new file/page/area) releases the optional worker.
    const observer = new MutationObserver(() => { if (!host.isConnected) controller.abort(); });
    observer.observe(document.body, { childList: true, subtree: true });
    try {
      const picture = available[Number(choice.value)];
      if (!picture) throw new Error(say("No extractable page image", "Немає доступного зображення сторінки"));
      const image = await createImageBitmap(picture.blob);
      const canvas = document.createElement("canvas");
      const scale = Math.min(1, 1600 / Math.max(image.width, image.height));
      canvas.width = Math.max(1, Math.round(image.width * scale)); canvas.height = Math.max(1, Math.round(image.height * scale));
      canvas.getContext("2d")?.drawImage(image, 0, 0, canvas.width, canvas.height); image.close();
      const found = await recognizeLocally(canvas, lang.value === "ukr" ? "ukr" : "eng", controller.signal);
      if (!host.isConnected) return;
      status.textContent = `${say("Suggested words", "Запропоновані слова")}: ${found.length}`;
      for (const suggestion of found) {
        const area = imageBoxOnPage(suggestion.box, picture.matrix);
        const row = el("div");
        const preview = el("button", "link", suggestion.text);
        preview.onclick = () => onPreview(area);
        const add = el("button", "btn", `${say("Add checked area", "Додати перевірену область")}: ${suggestion.text}`);
        add.onclick = () => onBox(area);
        row.append(preview, add); results.append(row);
      }
    } catch (error) { status.textContent = String(error); }
    finally { observer.disconnect(); run.disabled = choice.disabled = lang.disabled = false; cancel.hidden = true; }
  };
  host.append(choice, previewImage, lang, run, cancel, status, results);
  return host;
}
