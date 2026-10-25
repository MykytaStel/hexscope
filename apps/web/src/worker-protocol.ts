import type { PageArea, ParsedFile } from "./model";
import type { FilmScanReport } from "./filmscan";
import type { FilmSettings } from "./film-lab";
import type { FilmRenderResult } from "./film-render";
import type { MovieProgress } from "./movie";
import type { VerificationFinding, VerificationReport, VerificationSelection } from "./verification";

export type WorkerRequest =
  | { id: number; type: "parse"; file: File }
  | { id: number; type: "cancel"; target: number }
  | { id: number; type: "commitParse"; target: number }
  | { id: number; type: "steps"; from: number; count: number }
  | { id: number; type: "inflated" }
  | { id: number; type: "explain"; index: number; knownBlock: number }
  | { id: number; type: "selectEntry"; index: number }
  | { id: number; type: "open"; index: number }
  | { id: number; type: "openBytes"; bytes: Uint8Array }
  | { id: number; type: "back"; depth: number }
  | { id: number; type: "clean"; source: Blob; notes?: boolean; verify?: boolean }
  | {
      id: number;
      type: "verifyCopy";
      copy: Blob;
      sourceFormat: ParsedFile["format"];
      sourceFindings: VerificationFinding[];
      retainedReasons: Record<string, string>;
      sourceToken?: number;
      selections?: VerificationSelection[];
    }
  | { id: number; type: "repair"; bytes: Uint8Array }
  | { id: number; type: "filmRoll"; source: Blob; settings: FilmSettings; format: "jpeg" | "tiff16" }
  | { id: number; type: "filmRender"; source: Blob; dimensions: [number, number]; orientation: number; settings: FilmSettings; purpose: "preview" | "export"; point?: [number, number] }
  | { id: number; type: "pageTexts"; bytes: Uint8Array }
  | { id: number; type: "pagePictures"; bytes: Uint8Array; page: number }
  | { id: number; type: "redact"; bytes: Uint8Array; areas: Float64Array }
  | { id: number; type: "locate"; by: 0 | 1; pos: number }
  | { id: number; type: "blocks" }
  | { id: number; type: "codes"; picture?: Blob; pictureDimensions?: [number, number]; pdf?: Uint8Array; entries?: { index: number; name: string }[]; inspectFilmScan?: boolean };

export type OperationRequest = Exclude<WorkerRequest, { type: "cancel" | "commitParse" }>;

export type WorkerResponse =
  | { id: number; type: "parsed"; result: ParsedFile; bytes: Uint8Array }
  | { id: number; type: "opened"; result: ParsedFile; bytes: Uint8Array }
  | { id: number; type: "progress"; progress: MovieProgress }
  | { id: number; type: "back" }
  | { id: number; type: "cleaned"; copy: Blob; removed: { what: string; bytes: number }[]; orientation: number; error: string; verification?: string }
  | { id: number; type: "verification"; report: VerificationReport }
  | { id: number; type: "filmRollOutput"; result: import("./film-roll-worker").RollOutput }
  | { id: number; type: "filmRendered"; result: FilmRenderResult }
  | { id: number; type: "repaired"; bytes: Uint8Array; fixed: string[]; error: string }
  | { id: number; type: "pageTexts"; pages: PageGlyphs[] }
  | { id: number; type: "pagePictures"; pictures: PagePicture[] }
  | { id: number; type: "steps"; steps: Float64Array }
  | { id: number; type: "located"; step: Float64Array }
  | { id: number; type: "blocks"; map: Float64Array; note: string }
  | { id: number; type: "inflated"; bytes: Uint8Array }
  | { id: number; type: "explain"; parts: Float64Array; tables: Float64Array | null }
  | { id: number; type: "stream"; playable: boolean; trace: number[] | null; segments: Float64Array; idatBytes: number }
  | { id: number; type: "codes"; codes: { text: string; where: string; box: [number, number, number, number] }[]; filmScan?: FilmScanReport }
  | { id: number; type: "error"; message: string };

/** One page's visible glyphs: where each lands, four numbers each, and its text. */
export interface PageGlyphs {
  media: PageArea;
  areas: Float64Array;
  texts: string[];
  /** Whether every supported Form XObject on this page was inspected. */
  complete: boolean;
  /** The page's own dark boxes and marks, four numbers each. */
  boxes: Float64Array;
}

/** A picture a page draws: where its unit square goes, and a JPEG's bytes or its pixels. */
export interface PagePicture {
  matrix: number[];
  jpeg?: Uint8Array;
  width?: number;
  height?: number;
  rgba?: Uint8ClampedArray;
}
