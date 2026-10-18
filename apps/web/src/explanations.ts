// What each part of a file is, in one sentence, and where its format defines
// it. The parser's builds leave the sentences out — every visitor would
// download them first, most never read one — and name each by a key. They
// come from one file per build, written by the parser's own code
// (crates/hexscope-wasm/examples/explanations.rs), fetched the first time a
// file of that build is open and kept for the rest of the visit.
import type { ParsedFile } from "./model";

export interface Explanation {
  text: string;
  cite: string;
  url: string;
}

type Build = "media" | "pdf" | "docs";
type Written = Record<string, string | string[]>;

const FILES: Record<Build, () => Promise<{ default: Written }>> = {
  media: () => import("./explanations/explain-media.json"),
  pdf: () => import("./explanations/explain-pdf.json"),
  docs: () => import("./explanations/explain-docs.json"),
};

/** The formats each build beyond the pictures' reads (worker.ts, family.ts). */
const DOCS = new Set<ParsedFile["format"]>(["zip", "wasm", "eml", "msg", "office97", "cfb", "rtf"]);

const buildOf = (format: ParsedFile["format"]): Build =>
  format === "pdf" ? "pdf" : DOCS.has(format) ? "docs" : "media";

/** Every sentence fetched so far: keys are the same in every build's file. */
const known = new Map<number, Explanation>();
const loading: Partial<Record<Build, Promise<void>>> = {};

/** Fetches the sentences for a file of `format`; resolves at once when they are here. */
export function explain(format: ParsedFile["format"]): Promise<void> {
  const build = buildOf(format);
  return (loading[build] ??= FILES[build]().then(
    ({ default: written }) => {
      for (const [key, v] of Object.entries(written)) {
        known.set(Number(key), typeof v === "string" ? { text: v, cite: "", url: "" } : { text: v[0], cite: v[1], url: v[2] });
      }
    },
    (error: unknown) => {
      // Offline before the file was ever fetched: try again next time.
      delete loading[build];
      throw error;
    },
  ));
}

/** The sentence named `key`, once its file is here. */
export function explanation(key: number): Explanation | undefined {
  return known.get(key);
}
