/** Reading stages count bytes; indeterminate stages use a 0/1 placeholder. */
export type AnalysisProgressStage =
  | "read"
  | "load"
  | "scan"
  | "ranges"
  | "parse"
  | "summarize"
  | "entropy";

export interface AnalysisProgress {
  stage: AnalysisProgressStage;
  loaded: number;
  total: number;
  /** False while synchronous WebAssembly work cannot receive a cancel message. */
  cancellable: boolean;
  /** Movie-specific labels retained for its staged, range-based reader. */
  scope?: "video";
}
