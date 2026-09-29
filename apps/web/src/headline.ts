// The verdict in one line, large: what someone who opened the file wants to
// know before any detail — is something wrong, and how much does it say.
import type { FileModel } from "./model";
import { categories } from "./share";
import { verdict } from "./verdict";

export interface Headline {
  /** danger: act before sending; warning: worth a look; ok: nothing found; neutral: not read. */
  tone: "danger" | "warning" | "ok" | "neutral";
  text: string;
}

/** What gives a file away most: any one of these makes the headline red. */
const PRESSING = [
  "location", "photoplace", "covered", "hiddentext", "deleted", "earlier", "updates",
  "prompt", "linkmismatch", "linkidn", "riskyfile", "replyto", "authfail", "opens", "launch",
  "qrwifi", "qrsecret", "qrtrick", "qrplace",
];

/** What a file does when opened, rather than what it tells. */
const ACTIONS = ["opens", "launch", "scripts", "submits"];

/** A message that may not be from who it says. */
const FORGED = ["replyto", "authfail", "linkmismatch", "linkidn", "riskyfile", "qrtrick"];

/** What the file is, in a word a person would use. */
export function noun(m: FileModel): string {
  const f = m.file;
  switch (f.format) {
    case "jpeg":
    case "heif":
      return "photo";
    case "png":
    case "webp":
    case "gif":
      return "picture";
    case "video":
      return / audio\b/.test(m.value(0)) ? "recording" : "video";
    case "pdf":
      return "PDF";
    case "eml":
    case "msg":
      return "email";
    case "office97":
      return "document";
    case "wasm":
      return "module";
    case "zip":
      return ["word/document.xml", "xl/workbook.xml", "ppt/presentation.xml"].some((l) => f.labels.includes(l)) ? "document" : "archive";
    default:
      return "file";
  }
}

/** The answer as a row of a list says it, where the row names the file: "Gives away 4 things". */
export function listed(h: Headline): string {
  const t = h.text.replace(/^This \S+ (is )?/, "").replace(/ in this \S+$/, "");
  return t.charAt(0).toUpperCase() + t.slice(1);
}

export function headline(m: FileModel): Headline {
  const lines = verdict(m);
  if (m.file.format === "unknown") return { tone: "neutral", text: "Not a kind of file hexscope reads" };
  const n = noun(m);
  const kinds = new Set([...(m.file.location ? ["location"] : []), ...m.file.facts.map((f) => f.kind)]);
  const has = (k: string) => lines.some((l) => l.kind === k);
  if (has("damage")) return { tone: "danger", text: `This ${n} is damaged` };
  if (n === "email" && FORGED.some((k) => kinds.has(k))) {
    return { tone: "danger", text: "This email may not be from who it says" };
  }
  const count = categories(m).length;
  const pressing = has("hidden") || PRESSING.some((k) => kinds.has(k));
  if (count > 0) {
    return { tone: pressing ? "danger" : "warning", text: `This ${n} gives away ${count} ${count === 1 ? "thing" : "things"}` };
  }
  if (ACTIONS.some((k) => kinds.has(k))) return { tone: "danger", text: `This ${n} does more than show pages` };
  if (has("hidden")) return { tone: "danger", text: `Something is hidden in this ${n}` };
  if (has("misnamed")) return { tone: "warning", text: `This ${n} has the wrong name` };
  return { tone: "ok", text: `Nothing personal found in this ${n}` };
}
