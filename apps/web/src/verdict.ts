import { Concern, FileModel, Kind } from "./model";

/** One line of the verdict: what kind of finding, the sentence, where to look. */
export interface VerdictLine {
  kind: "damage" | "misnamed" | "hidden" | "warning" | "reveals" | "oddity" | "healthy" | "unknown";
  text: string;
  /** The node "Show me" selects, or -1 when there is nothing to point at. */
  node: number;
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** "a, b and c" */
export function list(items: string[]): string {
  if (items.length <= 1) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}

/** The extensions each format is named with. */
const EXTENSIONS: Record<string, string[]> = {
  jpeg: ["jpg", "jpeg", "jpe", "jfif"],
  png: ["png"],
  heif: ["heic", "heif", "hif", "avif"],
  webp: ["webp"],
  gif: ["gif"],
  video: ["mp4", "m4v", "mov", "qt", "3gp", "3g2", "m4a"],
  pdf: ["pdf"],
  zip: ["zip", "docx", "docm", "xlsx", "xlsm", "pptx", "pptm", "odt", "ods", "odp", "epub", "apk", "aab", "jar", "war", "xpi", "ipa", "whl", "nupkg", "kmz", "3mf", "usdz"],
  wasm: ["wasm"],
  eml: ["eml", "msg822", "mbox"],
  msg: ["msg"],
  office97: ["doc", "dot", "xls", "xlt", "xla", "ppt", "pot", "pps"],
};

/** A format as a person would name it, and the extension that fits it. */
const NAMES: Record<string, [string, string]> = {
  jpeg: ["a JPEG", ".jpg"],
  png: ["a PNG", ".png"],
  heif: ["a HEIF image (HEIC or AVIF)", ".heic"],
  webp: ["a WebP image", ".webp"],
  gif: ["a GIF", ".gif"],
  video: ["an MP4 or QuickTime movie", ".mp4"],
  pdf: ["a PDF", ".pdf"],
  zip: ["a ZIP archive", ".zip"],
  wasm: ["a WebAssembly module", ".wasm"],
  eml: ["an email", ".eml"],
  msg: ["an Outlook message", ".msg"],
  office97: ["a Word, Excel or PowerPoint 97–2003 file", ".doc"],
};

/** A name's extension that says another format than the bytes, what they are, and the extension that fits them. */
export function misfit(m: FileModel): { ext: string; what: string; fits: string } | null {
  const f = m.file;
  const dot = m.name.lastIndexOf(".");
  if (dot <= 0 || f.format === "unknown") return null;
  const ext = m.name.slice(dot + 1).toLowerCase();
  const claimed = Object.keys(EXTENSIONS).find((k) => EXTENSIONS[k].includes(ext));
  if (!claimed || claimed === f.format) return null;
  let [what, fits] = NAMES[f.format];
  // A HEIF says which it is: "HEIC · …" or "AVIF · …".
  const brand = f.format === "heif" ? m.value(0).split(" · ")[0] : "";
  if (brand === "HEIC" || brand === "AVIF") [what, fits] = [`a ${brand} image`, `.${brand.toLowerCase()}`];
  return { ext, what, fits };
}

/**
 * A file whose name says one format while its bytes are another: a HEIC
 * photo saved as .jpg, a PNG screenshot renamed. The bytes are fine; apps
 * that go by the name refuse it, which looks like damage and is not.
 */
function misnamed(m: FileModel): VerdictLine | null {
  const n = misfit(m);
  if (!n) return null;
  return {
    kind: "misnamed",
    text: `Named .${n.ext}, but it is ${n.what}. Apps that go by the name may refuse it or call it damaged; renaming it to ${n.fits} fixes that.`,
    // The bytes that say what it is: its signature.
    node: m.children(0)[0] ?? 0,
  };
}

/** Kinds named ahead of the rest in the "Reveals" line. */
const URGENT = ["hiddentext", "location", "earlier", "updates", "deleted", "photoplace"];

/** What each kind of fact gives away, in words, for the "Reveals" line. */
export const REVEALS: Record<string, string> = {
  location: "where it was taken",
  serial: "the camera's serial number",
  owner: "the owner's name",
  camera: "the camera",
  taken: "when it was taken",
  place: "the place it names",
  original: "the file it was made from",
  history: "how it was edited",
  author: "who wrote it",
  editor: "who saved it last",
  manager: "its manager's name",
  company: "the company",
  label: "the organisation it belongs to",
  downloaded: "where its files were downloaded from",
  created: "when it was written",
  editing: "how long it was worked on",
  updates: "earlier versions of itself",
  hiddentext: "text hidden from view",
  deleted: "text that was deleted",
  form: "what was filled into its form",
  attachments: "files attached to it",
  embedded: "whole files kept inside it",
  hiddensheets: "sheets hidden from view",
  hiddencells: "hidden rows and columns",
  links: "paths on the author's computer",
  notes: "the speaker's notes",
  hiddenslides: "hidden slides",
  earlier: "text an edit took off the page",
  photoplace: "where its photos were taken",
  photo: "the camera behind its photos",
  comments: "who commented",
  tracked: "who changed what",
  shutter: "how many photos the camera has taken",
  linked: "IDs that link it to other shots",
  uptime: "how long the phone had been on",
  paths: "the user name of the computer that built it",
  names: "its functions' names",
  toolchain: "the compiler that built it",
  screenshot: "that it is a screenshot",
  prompt: "the prompt it was made from",
  ai: "that AI made it",
  credentials: "who or what made it",
  sentfrom: "the address it was sent from",
  computer: "the sender's computer's name",
  mailer: "the mail app",
  timezone: "the sender's time zone",
  sourcemap: "where its source is",
  debug: "debug info from its source",
  qrwifi: "a Wi-Fi password, in a QR code",
  qrsecret: "a two-factor secret, in a QR code",
  qrplace: "a place, in a QR code",
  qrcontact: "contact details, in a QR code",
  qrtrick: "a QR code that hides where it goes",
  qrlink: "where a QR code leads",
};

/**
 * What hexscope found, in the order that matters to someone who is not a
 * format expert: damage first, then anything hidden, then what the file gives
 * away, then minor rule-breaking. Composed only from what the parse already
 * found; it is not a scan for malware, and says so where it is shown.
 */
/** What hexscope reads, for a file it does not. */
const READS = "it reads photos, videos, PDFs, Word, Excel and PowerPoint files, ZIP archives, emails and Outlook messages";

/** Kinds of file hexscope does not read, known by their first bytes: what to call one, and what to say. */
const OTHERS: [RegExp, string, string?][] = [
  [/^ID3|^\xff[\xfb\xf3\xf2]/, "an MP3 recording"],
  [/^RIFF....WAVE/s, "a WAV recording"],
  [/^OggS/, "an Ogg recording"],
  [/^fLaC/, "a FLAC recording"],
  [/^Rar!/, "a RAR archive"],
  [/^7z\xbc\xaf/, "a 7-Zip archive"],
  [/^\x1f\x8b/, "a gzip archive"],
  [/^\{\\rtf/, "a Rich Text document"],
  [/^%!PS/, "a PostScript file"],
  [/^BM/, "a BMP picture"],
  [/^(II\*\x00|MM\x00\*)/, "a TIFF picture or a camera's raw file"],
  [/^\x00\x00\x00\x0cjP/, "a JPEG 2000 picture"],
  [/^MZ/, "a Windows program"],
  [/^\x7fELF/, "a Linux program"],
  [/^(\xcf\xfa\xed\xfe|\xca\xfe\xba\xbe)/, "a Mac program"],
  [/^SQLite format 3/, "an SQLite database"],
];

/** A file hexscope does not read, named when its first bytes say what it is. */
function other(bytes: Uint8Array): string {
  const head = String.fromCharCode(...bytes.subarray(0, 16));
  const known = OTHERS.find(([re]) => re.test(head));
  if (!known) return `hexscope does not know this kind of file: ${READS}.`;
  const [, what, note] = known;
  return `This looks like ${what}. hexscope does not read it: ${READS}.${note ? ` ${note}` : ""}`;
}

export function verdict(m: FileModel): VerdictLine[] {
  const f = m.file;
  if (f.format === "unknown") {
    const message = m.children(0)[0];
    const label = message !== undefined ? m.label(message) : "";
    return [
      {
        kind: "unknown",
        text: label.startsWith("plain text")
          ? "Plain text: not a format hexscope takes apart. Its words read in the column beside the bytes."
          : label === "empty file"
            ? "The file is empty: there is not a single byte in it. A download or a copy may have stopped before it began."
            : other(m.bytes),
        node: message ?? -1,
      },
    ];
  }

  const concern = (id: number) => m.doc(id)?.concern ?? Concern.None;
  const damage = m.problems.filter((id) => m.kind(id) === Kind.Error || concern(id) === Concern.Damage);
  const hidden = m.problems.filter((id) => concern(id) === Concern.Hidden && !damage.includes(id));
  const odd = m.problems.filter((id) => !damage.includes(id) && !hidden.includes(id));

  const lines: VerdictLine[] = [];
  // Often the whole answer to "why won't it open", so it comes before any
  // damage; "looks healthy", which it agrees with, still goes above it.
  const name = misnamed(m);
  if (name) lines.push(name);
  if (damage.length > 0) {
    lines.push({
      kind: "damage",
      text: `Damaged: ${plural(damage.length, "place", "places")} could not be read properly. It may not open, or open only partly.`,
      node: damage[0],
    });
  }
  const email = f.format === "eml" || f.format === "msg";
  if (hidden.length > 0) {
    const more = hidden.length > 1 ? `, and ${plural(hidden.length - 1, "more thing", "more things")}` : "";
    lines.push({
      kind: "hidden",
      text: `${email ? "Possible signs of impersonation" : "Hidden in it"}: ${m.label(hidden[0])}${more}.`,
      node: hidden[0],
    });
  }

  const incompleteForm = f.facts.find((fact) => fact.kind === "form-incomplete");
  if (incompleteForm) {
    lines.push({
      kind: "warning",
      text: "Some PDF page or form content could not be fully checked. Search may miss text.",
      node: incompleteForm.node,
    });
  }

  const kinds = [...(f.location ? ["location"] : []), ...f.facts.map((x) => x.kind)];
  // What can hurt most goes first, so the four named include it.
  kinds.sort((a, b) => Number(URGENT.includes(b)) - Number(URGENT.includes(a)));
  const given = [...new Set(kinds.map((k) => REVEALS[k]).filter(Boolean))];
  if (given.length > 0) {
    // "Show me" goes to the first thing named.
    const top = kinds.find((k) => REVEALS[k]);
    const first = top === "location" ? (f.location?.node ?? -1) : (f.facts.find((x) => x.kind === top)?.node ?? -1);
    // A message is read by whoever got it: what it says is about its sender.
    const lead = email ? "Tells you about its sender: " : "Reveals ";
    lines.push({ kind: "reveals", text: `${lead}${list(given.slice(0, 4))}.`, node: first });
  }

  if (odd.length > 0 && damage.length === 0) {
    lines.push({
      kind: "oddity",
      text: `Breaks ${plural(odd.length, "rule", "rules")} of the format, usually harmlessly.`,
      node: odd[0],
    });
  }
  if (damage.length === 0 && hidden.length === 0 && odd.length === 0 && !incompleteForm) {
    // What it gives away is why someone opened it: health comes after that.
    if (lines.some((l) => l.kind === "reveals")) {
      lines.push({ kind: "healthy", text: "The file itself is fine: nothing in it is damaged.", node: -1 });
    } else {
      lines.unshift({ kind: "healthy", text: "The file is fine: nothing in it is damaged or out of place.", node: -1 });
    }
  }
  return lines;
}
