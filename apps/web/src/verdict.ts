import { Concern, FileModel, Kind } from "./model";

/** One line of the verdict: what kind of finding, the sentence, where to look. */
export interface VerdictLine {
  kind: "damage" | "misnamed" | "hidden" | "reveals" | "oddity" | "healthy" | "unknown";
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
  video: ["mp4", "m4v", "mov", "qt", "3gp", "3g2", "m4a"],
  pdf: ["pdf"],
  zip: ["zip", "docx", "docm", "xlsx", "xlsm", "pptx", "pptm", "odt", "ods", "odp", "epub", "apk", "aab", "jar", "war", "xpi", "ipa", "whl", "nupkg", "kmz", "3mf", "usdz"],
  wasm: ["wasm"],
};

/** A format as a person would name it, and the extension that fits it. */
const NAMES: Record<string, [string, string]> = {
  jpeg: ["a JPEG", ".jpg"],
  png: ["a PNG", ".png"],
  heif: ["a HEIF image (HEIC or AVIF)", ".heic"],
  video: ["an MP4 or QuickTime movie", ".mp4"],
  pdf: ["a PDF", ".pdf"],
  zip: ["a ZIP archive", ".zip"],
  wasm: ["a WebAssembly module", ".wasm"],
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
const URGENT = ["location", "earlier", "updates", "deleted", "photoplace"];

/** What each kind of fact gives away, in words, for the "Reveals" line. */
const REVEALS: Record<string, string> = {
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
  company: "the company",
  created: "when it was written",
  editing: "how long it was worked on",
  updates: "earlier versions of itself",
  deleted: "text that was deleted",
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
  sourcemap: "where its source is",
  debug: "debug info from its source",
};

/**
 * What hexscope found, in the order that matters to someone who is not a
 * format expert: damage first, then anything hidden, then what the file gives
 * away, then minor rule-breaking. Composed only from what the parse already
 * found; it is not a scan for malware, and says so where it is shown.
 */
export function verdict(m: FileModel): VerdictLine[] {
  const f = m.file;
  if (f.format === "unknown") {
    const message = m.children(0)[0];
    return [
      {
        kind: "unknown",
        text: `hexscope does not read this format${message !== undefined ? `: ${m.label(message)}` : ""}.`,
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
  if (hidden.length > 0) {
    const more = hidden.length > 1 ? `, and ${plural(hidden.length - 1, "more thing", "more things")}` : "";
    lines.push({
      kind: "hidden",
      text: `Something is hidden or disguised: ${m.label(hidden[0])}${more}.`,
      node: hidden[0],
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
    lines.push({ kind: "reveals", text: `Reveals ${list(given.slice(0, 4))}.`, node: first });
  }

  if (odd.length > 0 && damage.length === 0) {
    lines.push({
      kind: "oddity",
      text: `Breaks ${plural(odd.length, "rule", "rules")} of the format, usually harmlessly.`,
      node: odd[0],
    });
  }
  if (damage.length === 0 && hidden.length === 0 && odd.length === 0) {
    lines.unshift({
      kind: "healthy",
      text: "Looks healthy: every part reads the way the format says it should.",
      node: -1,
    });
  }
  return lines;
}
