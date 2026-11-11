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

export function pdfOcrTools(pictures: Promise<Shown[]>, onBox: (area: PageArea) => void): HTMLElement {
  const say = (en: string, uk: string) => currentLocale() === "uk" ? uk : en;
  const host = el("div", "pdf-ocr-tools");
  host.append(el("p", "hint", say("Experimental OCR reads only the first extractable picture on this page, not the complete PDF render. It may miss text. Check every suggested area before adding it. An optional engine and language model are loaded from this site; no picture is uploaded.", "Експериментальний OCR читає лише перше доступне зображення сторінки, а не повний рендер PDF. Він може пропустити текст. Перевіряйте кожну область перед додаванням. Рушій і мовна модель завантажуються з цього сайту; зображення не передається.")));
  const lang = el("select"); lang.setAttribute("aria-label", say("OCR language", "Мова OCR"));
  for (const [value, label] of [["eng", "English"], ["ukr", "Українська"]]) { const option = el("option", undefined, label); option.value = value; lang.append(option); }
  const run = el("button", "btn", say("Recognize the first page image locally", "Розпізнати перше зображення сторінки локально"));
  const cancel = el("button", "btn", say("Cancel recognition", "Скасувати розпізнавання")); cancel.hidden = true;
  const status = el("p", "hint"); status.setAttribute("role", "status");
  const results = el("div");
  let controller: AbortController;
  cancel.onclick = () => controller?.abort();
  run.onclick = async () => {
    run.disabled = true; cancel.hidden = false; controller = new AbortController();
    results.replaceChildren(); status.textContent = say("Recognizing text locally…", "Локальне розпізнавання тексту…");
    // Removing this preview (new file/page/area) releases the optional worker.
    const observer = new MutationObserver(() => { if (!host.isConnected) controller.abort(); });
    observer.observe(document.body, { childList: true, subtree: true });
    try {
      const picture = (await pictures)[0];
      if (!picture) throw new Error(say("No extractable page image", "Немає доступного зображення сторінки"));
      const image = await createImageBitmap(await (await fetch(picture.href)).blob());
      const canvas = document.createElement("canvas");
      const scale = Math.min(1, 1600 / Math.max(image.width, image.height));
      canvas.width = Math.max(1, Math.round(image.width * scale)); canvas.height = Math.max(1, Math.round(image.height * scale));
      canvas.getContext("2d")?.drawImage(image, 0, 0, canvas.width, canvas.height); image.close();
      const found = await recognizeLocally(canvas, lang.value === "ukr" ? "ukr" : "eng", controller.signal);
      if (!host.isConnected) return;
      status.textContent = `${say("Suggested words", "Запропоновані слова")}: ${found.length}`;
      for (const suggestion of found) {
        const add = el("button", "btn", `${say("Add checked area", "Додати перевірену область")}: ${suggestion.text}`);
        add.onclick = () => onBox(imageBoxOnPage(suggestion.box, picture.matrix));
        results.append(add);
      }
    } catch (error) { status.textContent = String(error); }
    finally { observer.disconnect(); run.disabled = false; cancel.hidden = true; }
  };
  host.append(lang, run, cancel, status, results);
  return host;
}
