import { createWorker, OEM } from "tesseract.js";

export interface OcrSuggestion { text: string; box: [number, number, number, number] }

/** Opt-in local OCR. Engine and models are same-origin, with no file uploads. */
export async function recognizeLocally(canvas: HTMLCanvasElement, lang: "eng" | "ukr", signal: AbortSignal): Promise<OcrSuggestion[]> {
  if (canvas.width * canvas.height > 4_000_000) throw new Error("OCR preview exceeds four million pixels");
  signal.throwIfAborted();
  const base = new URL("ocr-v7-data1/", document.baseURI).href;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let abort: () => void = () => {};
  const stopped = new Promise<never>((_, reject) => {
    abort = () => reject(new Error("OCR cancelled"));
    signal.addEventListener("abort", abort, { once: true });
    timer = setTimeout(() => reject(new Error("OCR exceeded its 60 second budget")), 60_000);
  });
  let closed = false;
  const pending = createWorker(lang, OEM.LSTM_ONLY, {
    workerPath: `${base}worker.min.js`, corePath: base, langPath: base.replace(/\/$/, ""),
    workerBlobURL: false, cacheMethod: "none",
  });
  // A model loading after cancellation still releases its worker.
  void pending.then((worker) => { if (closed) void worker.terminate(); }, () => {});
  let worker: Awaited<typeof pending> | undefined;
  try {
    worker = await Promise.race([pending, stopped]);
    const { data } = await Promise.race([worker.recognize(canvas, {}, { blocks: true, text: true }), stopped]);
    const words = data.blocks?.flatMap((block) => block.paragraphs.flatMap((paragraph) => paragraph.lines.flatMap((line) => line.words))) ?? [];
    return words.filter((word) => word.confidence >= 30 && word.text.trim()).slice(0, 200).map((word) => ({
      text: word.text.slice(0, 120),
      box: [Math.max(0, word.bbox.x0 / canvas.width - 0.004), Math.max(0, word.bbox.y0 / canvas.height - 0.004), Math.min(1, word.bbox.x1 / canvas.width + 0.004), Math.min(1, word.bbox.y1 / canvas.height + 0.004)],
    }));
  } finally {
    closed = true;
    if (timer) clearTimeout(timer);
    signal.removeEventListener("abort", abort);
    if (worker) await worker.terminate();
  }
}
