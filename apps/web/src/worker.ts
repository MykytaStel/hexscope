import { allowDocumentPrefetch, assertInputBudget, isFatalReaderError } from "./resource-budget";
import type { AnalysisProgress, AnalysisProgressStage } from "./analysis-progress";
// Parsing runs here so a large file never freezes the page. The parsed
// document stays alive in the worker so the DEFLATE player can ask for steps
// on demand instead of receiving millions of them up front.
import type { CleanCopy, Parsed } from "./wasm-pdf/hexscope_pdf.js";
import { type PageArea, type ParsedFile } from "./model";
import { describe, SEPARATOR } from "./describe";
import {
  assemble,
  LARGE_MOVIE,
  movieEntropy,
  readMovie,
  type Movie,
  type MovieReadOptions,
} from "./movie";
import { isPdf, paintJpegs } from "./paint";
import { familyOf, isMedia, type Family } from "./family";
import { PARSER_BUILDS } from "./parser-builds";
import { MAX_VERIFY_BYTES, decodeCoreVerification, uncheckedReport, verifyFileOutput, verifyPdfSelections, type VerificationPage, type VerificationReport } from "./verification";
import { FACT_LABELS } from "./knowledge";
import { verificationReasonMessage } from "./verification-reasons";
import { searchable } from "./redactor";
import { meaning } from "./qr/meaning";
import { scanCodes } from "./worker-qr-scan";
import { readEntry, selectEntry } from "./worker-entry-source";
import { createFilmQueue, handleFilmOperation, type FilmWorkerServices } from "./worker-film";
import type { OperationRequest, PageGlyphs, PagePicture, WorkerRequest, WorkerResponse } from "./worker-protocol";
export type { OperationRequest, PageGlyphs, PagePicture, WorkerRequest, WorkerResponse } from "./worker-protocol";

/**
 * The parser's builds, one per family of files: pictures, movies and SVG
 * drawings alone (what most files need), and each with PDF, or with Office,
 * email and WebAssembly, or audio tags. A file loads only the one that reads it (family.ts).
 */
type Module = typeof import("./wasm-pdf/hexscope_pdf.js");
const loaded: Partial<Record<Family, Promise<Module>>> = {};
/**
 * Why a parser could not be had, in words. A failed load is not kept: the
 * next file tries again, so a dropped connection is not the end of the tab.
 * A tab open across a new release asks for files the site no longer has.
 */
function unavailable(): Error {
  return new Error(
    navigator.onLine
      ? "hexscope was updated while this page was open. Reload the page to read it"
      : "you are offline, and this part of hexscope has not been saved for offline use yet",
  );
}
const load = (family: Family): Promise<Module> =>
  (loaded[family] ??= PARSER_BUILDS[family]()
    .then(async (imported) => {
      // The same interface in each, less what the other families need.
      const m = imported as Module;
      await m.default();
      return m;
    })
    .catch(() => {
      delete loaded[family];
      throw unavailable();
    }));
const loadMedia = () => load("media");
const loadPdf = () => load("pdf");

const moduleFor = (b: Uint8Array) => load(familyOf(b));

/**
 * Once a picture has been read, the document builds are fetched quietly —
 * not started — so the service worker has them for opening a document
 * offline. Not on a connection that asks to save data.
 */
let prefetched = false;
function prefetchDocuments(): void {
  if (prefetched || loaded.pdf || loaded.docs) return;
  prefetched = true;
  const network = navigator as Navigator & {
    connection?: { saveData?: boolean; effectiveType?: string };
    deviceMemory?: number;
  };
  const eligible = () => navigator.onLine && allowDocumentPrefetch(network.connection, network.deviceMemory);
  if (!eligible()) return;
  setTimeout(() => {
    if (!eligible()) return;
    if (!loaded.pdf) {
      void PARSER_BUILDS.pdf().catch(() => {});
      void fetch(new URL("./wasm-pdf/hexscope_pdf_bg.wasm", import.meta.url)).catch(() => {});
    }
    if (!loaded.docs) {
      void PARSER_BUILDS.docs().catch(() => {});
      void fetch(new URL("./wasm-docs/hexscope_docs_bg.wasm", import.meta.url)).catch(() => {});
    }
  }, 10_000);
}
/** The file, then each entry opened inside it; the last one is on screen. */
let stack: Parsed[] = [];
/** Source Blobs parallel to `stack`; nested files keep their ZIP bytes in WASM. */
let stackSources: (Blob | null)[] = [];
/** Active asynchronous parser and QR operations, cancellable by request ID. */
const activeOperations = new Map<number, AbortController>();
/** Parsed files wait here until the page accepts the result and commits it. */
const pendingParses = new Map<number, Parsed>();
const pendingSources = new Map<number, Blob | null>();
/** The open source parse used to compare the actual clean-copy bytes. */
const verificationSources = new Map<number, Parsed>();
const verificationTokens = new WeakMap<Parsed, number>();

function rememberParsedSource(parsed: Parsed, result: ParsedFile, token: number): void {
  const previous = verificationSources.get(token);
  previous?.free();
  verificationSources.set(token, parsed);
  verificationTokens.set(parsed, token);
  result.verificationToken = token;
}

function releaseParsedSource(parsed: Parsed): void {
  const token = verificationTokens.get(parsed);
  if (token === undefined) return;
  if (verificationSources.get(token) === parsed) verificationSources.delete(token);
  verificationTokens.delete(parsed);
}

function clearParsedStack(): void {
  for (const parsed of stack) {
    releaseParsedSource(parsed);
    parsed.free();
  }
  stack = [];
  stackSources = [];
}

function discardPendingParse(id: number): void {
  const parsed = pendingParses.get(id);
  pendingSources.delete(id);
  if (!parsed) return;
  pendingParses.delete(id);
  releaseParsedSource(parsed);
  parsed.free();
}

function commitPendingParse(id: number): void {
  const parsed = pendingParses.get(id);
  if (!parsed) return;
  pendingParses.delete(id);
  const source = pendingSources.get(id) ?? null;
  pendingSources.delete(id);
  clearParsedStack();
  stack = [parsed];
  stackSources = [source];
}

function stageParsed(
  id: number,
  parsed: Parsed,
  result: ParsedFile,
  bytes: Uint8Array,
  source: Blob | null,
): void {
  rememberParsedSource(parsed, result, id);
  pendingParses.set(id, parsed);
  pendingSources.set(id, source);
  try {
    post({ id, type: "parsed", result, bytes }, [...transfers(result), bytes.buffer]);
  } catch (error) {
    pendingParses.delete(id);
    pendingSources.delete(id);
    releaseParsedSource(parsed);
    throw error;
  }
}

const post = (msg: WorkerResponse, transfer: Transferable[] = []) =>
  (self as unknown as { postMessage(m: unknown, t: Transferable[]): void }).postMessage(
    msg,
    transfer,
  );

const transfers = (r: ParsedFile): Transferable[] => [
  r.starts.buffer,
  r.lens.buffer,
  r.parents.buffer,
  r.kinds.buffer,
  r.segments.buffer,
  r.entries.buffer,
  r.docIds.buffer,
  r.docKeys.buffer,
  r.docConcerns.buffer,
  r.composition.buffer,
  r.entropy.buffer,
  r.rowFilters.buffer,
  r.missing.buffer,
  ...(r.preview ? [r.preview.pixels.buffer] : []),
];

/** Bins the entropy minimap is drawn from; the page draws at most one per pixel row. */
const ENTROPY_BINS = 1024;
/** Keep file reads responsive to cancellation without issuing tiny reads. */
const FILE_READ_CHUNK = 4 * 1024 * 1024;

function reportProgress(
  id: number,
  stage: AnalysisProgressStage,
  loaded = 0,
  total = 1,
  cancellable = false,
  scope?: AnalysisProgress["scope"],
): void {
  const progress: AnalysisProgress = { stage, loaded, total, cancellable, ...(scope ? { scope } : {}) };
  post({ id, type: "progress", progress });
}

/** Reads a contiguous parser input while yielding between bounded file slices. */
async function readFileBytes(
  file: Blob,
  signal: AbortSignal,
  onProgress: (loaded: number, total: number) => void,
): Promise<Uint8Array> {
  const total = Math.max(1, file.size);
  onProgress(0, total);
  signal.throwIfAborted();
  const bytes = new Uint8Array(file.size);
  signal.throwIfAborted();
  if (file.size === 0) onProgress(1, total);
  for (let start = 0; start < file.size; start += FILE_READ_CHUNK) {
    signal.throwIfAborted();
    const end = Math.min(file.size, start + FILE_READ_CHUNK);
    const part = new Uint8Array(await file.slice(start, end).arrayBuffer());
    signal.throwIfAborted();
    bytes.set(part, start);
    onProgress(end, total);
  }
  return bytes;
}

/** A movie read in parts, when `file` is one large enough to need it. */
async function largeMovie(id: number, file: Blob, options?: MovieReadOptions): Promise<Movie | null> {
  if (file.size < LARGE_MOVIE) return null;
  reportProgress(id, "load", 0, 1, true);
  const wasm = await loadMedia();
  return readMovie(file, (head, len) => wasm.isMovie(head, len), LARGE_MOVIE, options);
}

async function parseFile(req: Extract<OperationRequest, { type: "parse" }>): Promise<void> {
  const controller = new AbortController();
  activeOperations.set(req.id, controller);
  const options: MovieReadOptions = {
    signal: controller.signal,
    onProgress: (progress) => post({ id: req.id, type: "progress", progress }),
  };
  try {
    const movie = await largeMovie(req.id, req.file, { ...options, includeWhole: false });
    controller.signal.throwIfAborted();
    if (movie) {
      const wasm = await loadMedia();
      controller.signal.throwIfAborted();
      reportProgress(req.id, "parse", 0, 1, false, "video");
      const t0 = performance.now();
      const parsed = wasm.parseMovie(movie.given, movie.gaps);
      let retained = false;
      try {
        reportProgress(req.id, "summarize", 0, 1, false, "video");
        const result = describe(parsed);
        parsed.releasePresentation();
        result.parseMs = performance.now() - t0;
        const e = await movieEntropy(req.file, movie, ENTROPY_BINS, (b) => wasm.entropy(b, 1)[0], options);
        result.entropy = e.values;
        result.entropyWindow = e.window;
        result.missing = movie.missing;
        result.compactSize = req.file.size;
        controller.signal.throwIfAborted();
        stageParsed(req.id, parsed, result, movie.given, null);
        retained = true;
      } finally {
        if (!retained) parsed.free();
      }
      return;
    }
    assertInputBudget(req.file.size);
    const bytes = await readFileBytes(req.file, controller.signal, (loaded, total) => {
      reportProgress(req.id, "read", loaded, total, true);
    });
    controller.signal.throwIfAborted();
    reportProgress(req.id, "load", 0, 1, true);
    const wasm = await moduleFor(bytes);
    controller.signal.throwIfAborted();
    reportProgress(req.id, "parse");
    // Timed from the call into WASM through reading every array back out, so
    // the number shown to the user is the whole cost, not the flattering part.
    const t0 = performance.now();
    const parsed = wasm.parseOwnedExternal(bytes);
    let retained = false;
    try {
      reportProgress(req.id, "summarize");
      const result = describe(parsed);
      parsed.releasePresentation();
      result.parseMs = performance.now() - t0;
      controller.signal.throwIfAborted();
      const shouldPrefetchDocuments = isMedia(bytes);
      // Source-dependent parsers keep this WASM allocation; the page receives
      // the JavaScript buffer by transfer, without another page-side copy.
      stageParsed(req.id, parsed, result, bytes, parsed.format === "zip" ? req.file : null);
      retained = true;
      if (shouldPrefetchDocuments) prefetchDocuments();
    } finally {
      if (!retained) parsed.free();
    }
  } finally {
    activeOperations.delete(req.id);
  }
}

/** Answers with a copy the parser made, as a file made by `wrap`. */
function postCopy(id: number, c: CleanCopy, wrap: (bytes: Uint8Array) => Blob, verify?: (bytes: Uint8Array) => string | undefined): void {
  try {
    const parts = c.removed.split(SEPARATOR);
    const removed = [];
    for (let i = 0; i + 1 < parts.length; i += 2) removed.push({ what: parts[i], bytes: Number(parts[i + 1]) });
    const error = c.error;
    const bytes = error ? new Uint8Array(0) : c.takeBytes();
    const verification = error ? undefined : verify?.(bytes);
    const copy = error ? new Blob([]) : wrap(bytes);
    post({ id, type: "cleaned", copy, removed, orientation: c.orientationKept, error, ...(verification ? { verification } : {}) });
  } finally {
    c.free();
  }
}

/** Does not touch WASM-backed stream state after the selected file has been freed. */
function postSelectedStream(id: number, current: Parsed, playable: boolean, isActive: boolean): void {
  if (!isActive) {
    const segments = new Float64Array();
    post({ id, type: "stream", playable: false, trace: null, segments, idatBytes: 0 }, [segments.buffer]);
    return;
  }
  const trace = Array.from(current.trace);
  const segments = current.segments;
  post({ id, type: "stream", playable, trace: trace.length ? trace : null, segments, idatBytes: current.idatBytes }, [
    segments.buffer,
  ]);
}

/** Keeps QR and embedded-image scanning outside the parser's central request router. */
function codes(req: Extract<OperationRequest, { type: "codes" }>, signal?: AbortSignal): ReturnType<typeof scanCodes> {
  const current = stack[stack.length - 1] ?? null;
  return scanCodes(
    req,
    signal ?? new AbortController().signal,
    current,
    loadPdf,
    stackSources[stackSources.length - 1] ?? null,
    () => current === null || stack[stack.length - 1] === current,
  );
}

const filmServices: FilmWorkerServices = { moduleFor, loadMedia, describe, codes, post };

/** Nested documents opened from archives are kept for the way back. */
const MAX_DEPTH = 4;

async function handle(req: OperationRequest): Promise<void> {
  if (req.type === "filmRoll" || req.type === "filmRender") {
    await handleFilmOperation(req, filmServices);
    return;
  }
  if (req.type === "parse") {
    await parseFile(req);
    return;
  }

  if (req.type === "copyPolicy") {
    if (req.copy.size > MAX_VERIFY_BYTES) throw new Error("Copy exceeds the verification limit");
    const bytes = new Uint8Array(await req.copy.arrayBuffer());
    const wasm = await moduleFor(bytes);
    const copy = wasm.parseOwned(bytes);
    try {
      post({ id: req.id, type: "policy", result: copy.evaluatePolicy(req.kinds, req.requireComplete) });
    } finally {
      copy.free();
    }
    return;
  }
  if (req.type === "policy") {
    const source = verificationSources.get(req.sourceToken);
    if (!source) throw new Error("Reopen the file to evaluate its policy");
    post({ id: req.id, type: "policy", result: source.evaluatePolicy(req.kinds, req.requireComplete) });
    return;
  }
  if (req.type === "clean") {
    const movie = await largeMovie(req.id, req.source, { includeWhole: false });
    if (movie) {
      const source = req.source;
      postCopy(req.id, (await loadMedia()).cleanMovie(movie.given, movie.gaps), (b) => assemble(b, movie, source));
      return;
    }
    assertInputBudget(req.source.size);
    const bytes = new Uint8Array(await req.source.arrayBuffer());
    const wasm = await moduleFor(bytes);
    // A PDF with a black box over a scanned JPEG: the picture is redrawn
    // with the box in it, as when blacking out by hand.
    let c = null;
    if (isPdf(bytes)) {
      const none = new Float64Array(0);
      const found = wasm.jpegsUnder(bytes, none);
      if (found.length > 0) {
        const painted = await paintJpegs(bytes, found);
        c = wasm.redactCopy(bytes, none, painted.nums, painted.lens, painted.bytes);
      }
    }
    c ??= wasm.cleanCopy(bytes, req.notes ?? false);
    postCopy(req.id, c, (b) => new Blob([b as BlobPart]), (copy) => {
      if (!req.verify || bytes.length > MAX_VERIFY_BYTES || copy.length > MAX_VERIFY_BYTES) return undefined;
      const parsed = wasm.parseOwned(bytes);
      try { return parsed.verifyCopy(copy); } finally { parsed.free(); }
    });
    return;
  }

  if (req.type === "verifyCopy") {
    const selections = req.selections ?? [];
    if (req.copy.size > MAX_VERIFY_BYTES) {
      post({
        id: req.id,
        type: "verification",
        report: uncheckedReport(req.sourceFindings, selections, "This copy is larger than the 10 MiB verification limit."),
      });
      return;
    }

    let pdfText: ReturnType<Module["pageTexts"]> | null = null;
    try {
      // Check the Blob size above before materialising another full copy of its bytes.
      const bytes = new Uint8Array(await req.copy.arrayBuffer());
      const wasm = await moduleFor(bytes);
      // Look up the token after awaiting bytes, then compare synchronously so a
      // newer parse cannot replace the source between lookup and comparison.
      const sourceParsed = req.sourceToken === undefined
        ? null
        : verificationSources.get(req.sourceToken) ?? null;
      const coreJson = sourceParsed?.verifyCopy(bytes) ?? null;
      let extraOutputFacts: { kind: string; text: string }[] = [];

      // QR findings are derived by the existing image scanner rather than the
      // format parser. Recheck only when the source had a code the cleaner keeps.
      const hasKeptCode = req.sourceFindings.some((finding) => finding.kind.startsWith("qr") && req.retainedReasons[finding.kind]);
      if (hasKeptCode && (req.sourceFormat === "pdf" || ["jpeg", "png", "heif", "webp", "gif"].includes(req.sourceFormat))) {
        try {
          const found = req.sourceFormat === "pdf"
            ? await codes({ id: req.id, type: "codes", pdf: bytes })
            : await codes({ id: req.id, type: "codes", picture: req.copy });
          extraOutputFacts = found.codes.map(({ text, where }) => {
            const fact = meaning(text);
            return { kind: fact.kind, text: where ? `${fact.text} (in ${where})` : fact.text };
          });
        } catch (error) {
          if (isFatalReaderError(error)) throw error;
          // The copy can still be checked for parser-backed findings; QR facts
          // without a second scan remain unchecked by the capability table.
        }
      }

      const parserFindings = req.sourceFindings.filter((finding) => !finding.kind.startsWith("qr"));
      const parserReport = coreJson
        ? decodeCoreVerification(coreJson, req.sourceFormat, FACT_LABELS, req.retainedReasons)
        : null;
      const safeParserReport = parserReport ?? uncheckedReport(
        parserFindings,
        [],
        sourceParsed
          ? verificationReasonMessage("parse_incomplete", "")!
          : verificationReasonMessage("verification_skipped", "")!,
      );

      const qrFindings = req.sourceFindings.filter((finding) => finding.kind.startsWith("qr"));
      const qrReport = qrFindings.length || extraOutputFacts.length
        ? verifyFileOutput(
          qrFindings,
          { format: req.sourceFormat, facts: [], location: null },
          FACT_LABELS,
          req.retainedReasons,
          extraOutputFacts,
        )
        : { removed: [], present: [], unchecked: [] };

      let textReport: VerificationReport = { removed: [], present: [], unchecked: [] };
      if (req.sourceFormat === "pdf" && selections.length) {
        pdfText = wasm.pageTexts(bytes);
        const pages: PageGlyphs[] = [];
        for (let i = 0; i < pdfText.count; i++) {
          const texts = pdfText.texts(i);
          pages.push({
            media: Array.from(pdfText.media(i)) as PageArea,
            areas: pdfText.areas(i),
            texts: texts ? texts.split(SEPARATOR) : [],
            complete: pdfText.complete(i),
            boxes: pdfText.boxes(i),
          });
        }
        const searchablePages: VerificationPage[] = searchable(pages).map(({ page, text, complete }) => ({ page, text, complete }));
        textReport = verifyPdfSelections(selections, searchablePages);
      }

      const reports = [safeParserReport, qrReport, textReport];
      const hasSpecificResult = reports.some((report) =>
        [...report.removed, ...report.present, ...report.unchecked].some((finding) => finding.kind !== "verification"),
      );
      const visibleReports = hasSpecificResult
        ? reports.map((report) => ({
          removed: report.removed.filter((finding) => finding.kind !== "verification"),
          present: report.present.filter((finding) => finding.kind !== "verification"),
          unchecked: report.unchecked.filter((finding) => finding.kind !== "verification"),
        }))
        : reports;
      const report: VerificationReport = {
        removed: visibleReports.flatMap((part) => part.removed),
        present: visibleReports.flatMap((part) => part.present),
        unchecked: visibleReports.flatMap((part) => part.unchecked),
      };
      post({ id: req.id, type: "verification", report });
    } catch (error) {
      if (isFatalReaderError(error)) throw error;
      post({
        id: req.id,
        type: "verification",
        report: uncheckedReport(req.sourceFindings, selections, "hexscope could not completely read the copy, so its findings were not checked."),
      });
    } finally {
      try {
        pdfText?.free();
      } catch {
        // A crashed Wasm parser may no longer be able to free its temporary page table.
      }
    }
    return;
  }

  if (req.type === "pageTexts") {
    const t = (await loadPdf()).pageTexts(req.bytes);
    try {
      const pages: PageGlyphs[] = [];
      for (let i = 0; i < t.count; i++) {
        const texts = t.texts(i);
        pages.push({ page: i + 1, object: t.object(i), media: Array.from(t.media(i)) as PageArea, areas: t.areas(i), texts: texts ? texts.split(SEPARATOR) : [], complete: t.complete(i), boxes: t.boxes(i) });
      }
      post({ id: req.id, type: "pageTexts", pages });
    } finally {
      t.free();
    }
    return;
  }

  if (req.type === "pagePictures") {
    const t = (await loadPdf()).pagePictures(req.bytes, req.page);
    try {
      const pictures: PagePicture[] = [];
      const moved: ArrayBuffer[] = [];
      for (let i = 0; i < t.count; i++) {
        const matrix = Array.from(t.matrix(i));
        const at = t.jpeg(i);
        if (at.length === 2) {
          const jpeg = req.bytes.slice(at[0], at[0] + at[1]);
          moved.push(jpeg.buffer);
          pictures.push({ matrix, jpeg });
          continue;
        }
        const [width, height] = t.size(i);
        const rgba = new Uint8ClampedArray(t.rgba(i).buffer);
        moved.push(rgba.buffer as ArrayBuffer);
        pictures.push({ matrix, width, height, rgba });
      }
      post({ id: req.id, type: "pagePictures", pictures }, moved);
    } finally {
      t.free();
    }
    return;
  }

  if (req.type === "redact") {
    const wasm = await loadPdf();
    const painted = await paintJpegs(req.bytes, wasm.jpegsUnder(req.bytes, req.areas));
    const c = wasm.redactCopy(req.bytes, req.areas, painted.nums, painted.lens, painted.bytes);
    postCopy(req.id, c, (b) => new Blob([b as BlobPart]));
    return;
  }

  if (req.type === "repair") {
    const r = (await moduleFor(req.bytes)).repairCopy(req.bytes);
    try {
      const bytes = r.bytes;
      post({ id: req.id, type: "repaired", bytes, fixed: r.fixed ? r.fixed.split(SEPARATOR) : [], error: r.error }, [bytes.buffer]);
    } finally {
      r.free();
    }
    return;
  }

  if (req.type === "codes") {
    const controller = new AbortController();
    activeOperations.set(req.id, controller);
    try {
      const result = await codes(req, controller.signal);
      controller.signal.throwIfAborted();
      post({ id: req.id, type: "codes", ...result });
    } finally {
      activeOperations.delete(req.id);
    }
    return;
  }

  if (stack.length === 0) throw new Error("No file is open");
  const current = stack[stack.length - 1];
  if (req.type === "open" || req.type === "openBytes") {
    if (stack.length > MAX_DEPTH) throw new Error(`files nest at most ${MAX_DEPTH} deep here`);
    // An archive's entry, or a file found inside the bytes.
    const bytes = req.type === "open"
      ? await readEntry(
        current,
        stackSources[stackSources.length - 1] ?? null,
        req.index,
        () => stack[stack.length - 1] === current,
      )
      : req.bytes;
    if (req.type === "open" && stack[stack.length - 1] !== current) {
      throw new Error("the file changed while its archive entry was being read");
    }
    if (bytes.length === 0) throw new Error(`This entry cannot be opened: ${current.extractError}.`);
    // Its own build: a ZIP behind a photo needs the one that reads archives.
    const wasm = await moduleFor(bytes);
    if (req.type === "open" && stack[stack.length - 1] !== current) {
      throw new Error("the file changed while its archive entry was being read");
    }
    const t0 = performance.now();
    if (stack[stack.length - 1] !== current) throw new Error("the file changed while its archive entry was being read");
    assertInputBudget(bytes.length);
    const parsed = wasm.parseOwned(bytes);
    let retained = false;
    try {
      const result = describe(parsed);
      parsed.releasePresentation();
      result.parseMs = performance.now() - t0;
      rememberParsedSource(parsed, result, req.id);
      post({ id: req.id, type: "opened", result, bytes }, [...transfers(result), bytes.buffer]);
      stack.push(parsed);
      stackSources.push(null);
      retained = true;
    } finally {
      if (!retained) {
        releaseParsedSource(parsed);
        parsed.free();
      }
    }
    return;
  }
  if (req.type === "back") {
    // Never below the file itself.
    while (stack.length > Math.max(1, req.depth + 1)) {
      const parsed = stack.pop()!;
      releaseParsedSource(parsed);
      parsed.free();
      stackSources.pop();
    }
    post({ id: req.id, type: "back" });
    return;
  }
  if (req.type === "blocks") {
    const map = current.blockMap();
    post({ id: req.id, type: "blocks", map, note: current.blocksNote }, [map.buffer]);
  } else if (req.type === "locate") {
    const step = current.locate(req.by, req.pos);
    post({ id: req.id, type: "located", step }, [step.buffer]);
  } else if (req.type === "steps") {
    const steps = current.steps(req.from, req.count);
    post({ id: req.id, type: "steps", steps }, [steps.buffer]);
  } else if (req.type === "selectEntry") {
    const playable = await selectEntry(
      current,
      stackSources[stackSources.length - 1] ?? null,
      req.index,
      () => stack[stack.length - 1] === current,
    );
    postSelectedStream(req.id, current, playable, stack[stack.length - 1] === current);
  } else if (req.type === "explain") {
    const parts = current.explain(req.index);
    // Tables change once per block: send them only when the page's are stale.
    const tables = parts.length > 0 && parts[0] !== req.knownBlock ? current.tables(req.index) : null;
    post({ id: req.id, type: "explain", parts, tables }, tables ? [parts.buffer, tables.buffer] : [parts.buffer]);
  } else {
    const bytes = current.inflated;
    post({ id: req.id, type: "inflated", bytes }, [bytes.buffer]);
  }
}

let readerPoisoned = false;
function answer(req: OperationRequest): Promise<void> {
  if (readerPoisoned) return Promise.resolve();
  return handle(req).catch((err: unknown) => {
    // Out of memory, the WebAssembly stops for good: start the next file
    // on a fresh one, and say what happened in words.
    const text = err instanceof Error ? err.message : String(err);
    const crashed = isFatalReaderError(err);
    if (crashed) readerPoisoned = true;
    post({
      id: req.id,
      type: "error",
      message: crashed ? "it needs more memory than this browser tab has. Close other tabs and try again, or use the command line tool" : text,
      ...(crashed ? { fatal: true as const } : {}),
    });
    // Removing entries from the JS module map cannot unload a WASM instance.
    // Stop accepting work until the owning page terminates this worker.
    if (crashed) self.close();
  });
}
const filmQueue = createFilmQueue((req) => answer(req), (req) => {
  post({ id: req.id, type: "error", message: "A newer film lab request replaced this preview." });
});
self.onmessage = (event: MessageEvent<WorkerRequest>) => {
  if (readerPoisoned) return;
  if (event.data.type === "cancel") {
    activeOperations.get(event.data.target)?.abort();
    discardPendingParse(event.data.target);
    return;
  }
  if (event.data.type === "commitParse") {
    commitPendingParse(event.data.target);
    return;
  }
  if (event.data.type === "filmRender" || event.data.type === "filmRoll") filmQueue.push(event.data);
  else void answer(event.data);
};
