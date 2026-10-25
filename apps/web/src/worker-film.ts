import { LatestFilmQueue } from "./film-queue";
import { isMedia } from "./family";
import { SEPARATOR } from "./describe";
import { profileFromTree } from "./film-profile";
import { FACT_LABELS } from "./knowledge";
import { MAX_VERIFY_BYTES } from "./verification";
import type { ParsedFile } from "./model";
import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";
import type { OperationRequest, WorkerResponse } from "./worker-protocol";

type Module = typeof import("./wasm-pdf/hexscope_pdf.js");
type FilmCodesRequest = Extract<OperationRequest, { type: "codes" }>;
type FilmCodesResult = ReturnType<typeof import("./worker-qr-scan").scanCodes>;

export type FilmOperationRequest = Extract<OperationRequest, { type: "filmRoll" | "filmRender" }>;

export interface FilmWorkerServices {
  moduleFor(bytes: Uint8Array): Promise<Module>;
  loadMedia(): Promise<Module>;
  describe(parsed: Parsed): ParsedFile;
  codes(request: FilmCodesRequest): FilmCodesResult;
  post(response: WorkerResponse): void;
}

type FilmRollRequest = Extract<FilmOperationRequest, { type: "filmRoll" }>;
type FilmRenderRequest = Extract<FilmOperationRequest, { type: "filmRender" }>;
type FilmRenderResult = Extract<WorkerResponse, { type: "filmRendered" }>["result"];

export function createFilmQueue(
  run: (request: FilmOperationRequest) => Promise<void>,
  discard: (request: FilmOperationRequest) => void,
): LatestFilmQueue<FilmOperationRequest> {
  return new LatestFilmQueue(run, discard);
}

export async function handleFilmOperation(request: FilmOperationRequest, services: FilmWorkerServices): Promise<void> {
  if (request.type === "filmRoll") return filmRoll(request, services);
  return filmRender(request, services);
}

async function filmRoll(request: FilmRollRequest, services: FilmWorkerServices): Promise<void> {
  if (request.source.size > 50 * 1024 * 1024) throw new Error("This scan exceeds the 50 MiB file limit.");
  const head = new Uint8Array(await request.source.slice(0, 12).arrayBuffer());
  let dimensions: [number, number] | undefined;
  let orientation = 1;
  let profilePresent: boolean | null = null;
  if (isMedia(head)) {
    const wasm = await services.moduleFor(head);
    const parsed = wasm.parseOwned(new Uint8Array(await request.source.arrayBuffer()));
    try {
      if (parsed.dimensions.length === 2) dimensions = [parsed.dimensions[0], parsed.dimensions[1]];
      orientation = parsed.orientation;
      profilePresent = profileFromTree({
        format: parsed.format as ParsedFile["format"],
        labels: parsed.labels.split(SEPARATOR),
        kinds: parsed.kinds,
      });
    } finally {
      parsed.free();
    }
  }
  const { renderRoll } = await import("./film-roll-worker");
  const result = await renderRoll(request.source, request.settings, request.format, dimensions, orientation, profilePresent);
  services.post({ id: request.id, type: "filmRollOutput", result });
}

async function filmRender(request: FilmRenderRequest, services: FilmWorkerServices): Promise<void> {
  const { renderFilm } = await import("./film-render");
  const result = await renderFilm(request.source, request.dimensions, request.orientation, request.settings, request.purpose, request.point);
  if (request.purpose === "export") await checkFilmExport(request.id, result, services);
  services.post({ id: request.id, type: "filmRendered", result });
}

async function checkFilmExport(id: number, result: FilmRenderResult, services: FilmWorkerServices): Promise<void> {
  result.check = { status: "unavailable", metadata: [], qrCount: null };
  try {
    const bytes = await readFilmExport(result, services);
    const inspected = await services.codes({
      id,
      type: "codes",
      picture: result.copy,
      pictureDimensions: result.dimensions,
      inspectFilmScan: true,
    });
    result.check.scan = inspected.filmScan;
    result.check.qrCount = inspected.codes.length > 0 || inspected.filmScan?.availability === "inspected" ? inspected.codes.length : null;
    result.check.sha256 = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)))
      .map((value) => value.toString(16).padStart(2, "0")).join("");
  } catch {
    // Preserve the produced copy; a failed readback never becomes a clean claim.
  }
}

async function readFilmExport(result: FilmRenderResult, services: FilmWorkerServices): Promise<Uint8Array<ArrayBuffer>> {
  if (result.copy.size > MAX_VERIFY_BYTES) throw new Error("copy exceeds readback limit");
  const wasm = await services.loadMedia();
  // Canvas encodes fresh pixels, without copying source metadata segments.
  // Read the actual result; the cleaner correctly refuses an already-clean file.
  const bytes = new Uint8Array(await result.copy.arrayBuffer());
  const parsed = wasm.parseOwned(bytes);
  try {
    const file = services.describe(parsed);
    if (file.format !== "jpeg" || file.kinds.includes(3) || file.dimensions?.[0] !== result.dimensions[0] || file.dimensions?.[1] !== result.dimensions[1]) {
      throw new Error("copy cannot be completely read");
    }
    result.check = {
      status: "checked",
      metadata: [...file.facts.map((fact) => FACT_LABELS[fact.kind] ?? fact.kind), ...(file.location ? ["Location"] : [])],
      qrCount: null,
    };
  } finally {
    parsed.free();
  }
  return bytes;
}
