import { Concern, FileModel, Kind } from "./model";

/** One line of the verdict: what kind of finding, the sentence, where to look. */
export interface VerdictLine {
  kind: "damage" | "hidden" | "reveals" | "oddity" | "healthy" | "unknown";
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

/** What each kind of fact gives away, in words, for the "Reveals" line. */
const REVEALS: Record<string, string> = {
  location: "where it was taken",
  serial: "the camera's serial number",
  owner: "the owner's name",
  camera: "the camera",
  taken: "when it was taken",
  author: "who wrote it",
  editor: "who saved it last",
  company: "the company",
  created: "when it was written",
  editing: "how long it was worked on",
  updates: "earlier versions of itself",
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
  const given = [...new Set(kinds.map((k) => REVEALS[k]).filter(Boolean))];
  if (given.length > 0) {
    const first = f.location?.node ?? f.facts[0]?.node ?? -1;
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
