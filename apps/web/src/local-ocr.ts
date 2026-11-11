export interface OcrSuggestion { text: string; box: [number, number, number, number] }

/** Opt-in local OCR. Engine and models are same-origin, with no file uploads. */
export async function recognizeLocally(canvas: HTMLCanvasElement, lang: "eng" | "ukr", signal: AbortSignal): Promise<OcrSuggestion[]> {
  if (canvas.width * canvas.height > 4_000_000) throw new Error("OCR preview exceeds four million pixels");
  signal.throwIfAborted();
  const base = new URL("ocr-v7-data1/", document.baseURI).href;
  const bytes = await new Promise<Blob>((resolve, reject) => canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("Could not encode OCR preview")), "image/png")).then((blob) => blob.arrayBuffer());
  signal.throwIfAborted();
  // Owning the outer worker makes cancellation/deadlines apply during model
  // initialization too, before Tesseract's createWorker promise has resolved.
  const worker = new Worker(new URL("./ocr-worker.ts", import.meta.url), { type: "module" });
  let timer: ReturnType<typeof setTimeout> | undefined;
  let abort: () => void = () => {};
  try {
    return await new Promise<OcrSuggestion[]>((resolve, reject) => {
      abort = () => reject(new Error("OCR cancelled"));
      signal.addEventListener("abort", abort, { once: true });
      timer = setTimeout(() => reject(new Error("OCR exceeded its 60 second budget")), 60_000);
      worker.addEventListener("message", (event: MessageEvent<{ suggestions?: OcrSuggestion[]; error?: string }>) => {
        if (event.data.error) reject(new Error(event.data.error));
        else resolve(event.data.suggestions ?? []);
      });
      worker.addEventListener("error", () => reject(new Error("The local OCR reader stopped")));
      worker.addEventListener("messageerror", () => reject(new Error("The local OCR reader could not return its result")));
      worker.postMessage({ bytes, width: canvas.width, height: canvas.height, lang, base }, [bytes]);
    });
  } finally {
    if (timer) clearTimeout(timer);
    signal.removeEventListener("abort", abort);
    worker.terminate();
  }
}
