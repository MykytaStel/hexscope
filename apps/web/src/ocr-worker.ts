import { createWorker, OEM } from "tesseract.js";
import type { OcrSuggestion } from "./local-ocr";

/** Disposable owner of the OCR engine, including its initialization phase. */
self.onmessage = async (event: MessageEvent<{ bytes: ArrayBuffer; width: number; height: number; lang: "eng" | "ukr"; base: string }>) => {
  const { bytes, width, height, lang, base } = event.data;
  let worker: Awaited<ReturnType<typeof createWorker>> | undefined;
  try {
    worker = await createWorker(lang, OEM.LSTM_ONLY, {
      workerPath: `${base}worker.min.js`, corePath: base, langPath: base.replace(/\/$/, ""),
      workerBlobURL: false, cacheMethod: "none",
    });
    const { data } = await worker.recognize(new Blob([bytes], { type: "image/png" }), {}, { blocks: true, text: true });
    const words = data.blocks?.flatMap((block) => block.paragraphs.flatMap((paragraph) => paragraph.lines.flatMap((line) => line.words))) ?? [];
    const suggestions: OcrSuggestion[] = words.filter((word) => word.confidence >= 30 && word.text.trim()).slice(0, 200).map((word) => ({
      text: word.text.slice(0, 120),
      box: [Math.max(0, word.bbox.x0 / width - 0.004), Math.max(0, word.bbox.y0 / height - 0.004), Math.min(1, word.bbox.x1 / width + 0.004), Math.min(1, word.bbox.y1 / height + 0.004)],
    }));
    self.postMessage({ suggestions });
  } catch { self.postMessage({ error: "Could not recognize this preview locally" }); }
  finally { if (worker) await worker.terminate(); }
};
