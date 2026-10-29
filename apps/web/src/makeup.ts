// "What it's made of": every byte of the file as the thing itself, what is
// about it, structure, and what the format does not account for — in one
// sentence, a bar in file order, and a row per kind.
import { Role, type FileModel, type Slice } from "./model";
import { el, formatBytes } from "./dom";
import { roleName } from "./knowledge";

/** Parts the bar draws, at most: past a few hundred, a part is narrower than a pixel. */
export const MAX_BAR_PARTS = 400;

/** One stretch of the bar: its colour, its bytes, and the part a click shows (-1: none). */
export interface BarPart {
  role: number;
  len: number;
  node: number;
  /** Drawn for more than one slice of the file. */
  merged: boolean;
}

/** A part being gathered, with the size of the slice its click leads to. */
type Gathered = BarPart & { lead: number };

/** Keeps the larger slice that leads somewhere as the part's click. */
function lead(into: Gathered, len: number, node: number): void {
  if (node >= 0 && len > into.lead) {
    into.node = node;
    into.lead = len;
  }
}

function joinNeighbours(parts: Gathered[]): Gathered[] {
  const out: Gathered[] = [];
  for (const p of parts) {
    const last = out[out.length - 1];
    if (last?.role !== p.role) out.push({ ...p });
    else {
      last.len += p.len;
      last.merged = true;
      lead(last, p.lead, p.node);
    }
  }
  return out;
}

/**
 * The bar's parts in file order: a slice each for an ordinary file. A large
 * archive has two slices an entry; past `max`, neighbours of one colour are
 * one part, and if that is still too many, the file is cut into `max / 2`
 * equal stretches, each drawn as its colours in proportion — the bar at a
 * lower resolution, the whole file in a few hundred elements, not one per slice.
 */
export function barParts(slices: readonly Slice[], max = MAX_BAR_PARTS): BarPart[] {
  if (slices.length <= max) return slices.map((s) => ({ role: s.role, len: s.len, node: s.node, merged: false }));
  const runs = joinNeighbours(slices.map((s) => ({ role: s.role, len: s.len, node: s.node, merged: false, lead: s.node >= 0 ? s.len : -1 })));
  if (runs.length > max) {
    const total = slices.reduce((n, s) => n + s.len, 0);
    const count = Math.max(1, Math.floor(max / 2));
    const stretches: Gathered[] = [];
    // Bytes per colour, in the order met, in the stretch being read; it ends at `end`, the `b`th of `count`.
    let bytes = new Map<number, Gathered>();
    let at = 0, b = 0, end = Math.round(total / count);
    const close = () => {
      stretches.push(...bytes.values());
      bytes = new Map();
    };
    for (const s of slices) {
      let left = s.len;
      while (left > 0) {
        const piece = Math.min(left, end - at);
        if (piece > 0) {
          const g = bytes.get(s.role) ?? { role: s.role, len: 0, node: -1, merged: true, lead: -1 };
          g.len += piece;
          lead(g, s.len, s.node);
          bytes.set(s.role, g);
        }
        at += piece;
        left -= piece;
        if (at >= end && b < count - 1) {
          close();
          end = Math.round((total * (++b + 1)) / count);
        }
      }
    }
    close();
    return joinNeighbours(stretches).map(({ lead: _, ...p }) => p);
  }
  return runs.map(({ lead: _, ...p }) => p);
}

export function makeupGroup(m: FileModel, select: (id: number) => void, hover: (id: number) => void): HTMLElement {
  const group = el("div", "group makeup");
  group.append(el("h2", undefined, "What it's made of"));
  const slices = m.slices();
  const total = slices.reduce((n, s) => n + s.len, 0);
  if (total === 0) return group;

  const bar = el("div", "makeup-bar");
  for (const p of barParts(slices)) {
    const part = el("span", `makeup-part role-${p.role}`);
    part.style.flexGrow = String(p.len);
    part.title = `${roleName(p.role, m.file.format)}${p.node >= 0 && !p.merged ? ` · ${m.label(p.node)}` : ""} · ${formatBytes(p.len)}`;
    if (p.node >= 0) {
      part.addEventListener("click", () => select(p.node));
      part.addEventListener("mouseenter", () => hover(p.node));
      part.addEventListener("mouseleave", () => hover(-1));
    }
    bar.append(part);
  }

  // Per role: bytes, and the largest slice to jump to.
  const byRole = new Map<number, { bytes: number; biggest: (typeof slices)[number] }>();
  for (const s of slices) {
    const r = byRole.get(s.role);
    if (!r) byRole.set(s.role, { bytes: s.len, biggest: s });
    else {
      r.bytes += s.len;
      if (s.len > r.biggest.len) r.biggest = s;
    }
  }
  const legend = el("ul", "makeup-legend");
  for (const [role, r] of [...byRole].sort((a, b) => b[1].bytes - a[1].bytes)) {
    const pct = (r.bytes / total) * 100;
    const li = el("li", "makeup-row");
    li.append(
      el("span", `makeup-swatch role-${role}`),
      el("span", "makeup-name", roleName(role, m.file.format)),
      el("span", "makeup-pct", pct < 1 ? "<1%" : `${Math.round(pct)}%`),
      el("span", "makeup-size", formatBytes(r.bytes)),
    );
    if (r.biggest.node >= 0) {
      li.classList.add("is-link");
      li.title = "Show the largest part";
      li.addEventListener("click", () => select(r.biggest.node));
    }
    legend.append(li);
  }
  // In one sentence: the thing itself, what is about it, what is unaccounted.
  const share = (role: number) => (byRole.get(role)?.bytes ?? 0) / total;
  const pct = (x: number) => (x < 0.01 ? "under 1%" : `${Math.round(x * 100)}%`);
  const about = share(Role.Metadata) + share(Role.Thumbnail);
  const parts = [`${pct(share(Role.Content))} goes to the ${roleName(Role.Content, m.file.format).toLowerCase().replace(/^the /, "")}`];
  if (about > 0) parts.push(`${pct(about)} to information about it`);
  if (share(Role.Structure) >= 0.01) parts.push(`${pct(share(Role.Structure))} to the structure that holds it together`);
  if (share(Role.Hidden) > 0) parts.push(`${pct(share(Role.Hidden))} to bytes the format does not account for`);
  const thumb = share(Role.Thumbnail) > 0 ? " — a second, small copy of the picture among it" : "";
  const said = `Of its ${formatBytes(total)}, ${parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}` : parts[0]}${thumb}.`;
  group.append(el("p", "makeup-said", said), bar, legend);
  return group;
}
