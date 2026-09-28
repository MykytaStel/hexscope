// "What it's made of": every byte of the file as the thing itself, what is
// about it, structure, and what the format does not account for — in one
// sentence, a bar in file order, and a row per kind.
import { Role, type FileModel } from "./model";
import { el, formatBytes } from "./dom";
import { roleName } from "./knowledge";

export function makeupGroup(m: FileModel, select: (id: number) => void, hover: (id: number) => void): HTMLElement {
  const group = el("div", "group makeup");
  group.append(el("h2", undefined, "What it's made of"));
  const slices = m.slices();
  const total = slices.reduce((n, s) => n + s.len, 0);
  if (total === 0) return group;

  const bar = el("div", "makeup-bar");
  for (const s of slices) {
    const part = el("span", `makeup-part role-${s.role}`);
    part.style.flexGrow = String(s.len);
    part.title = `${roleName(s.role, m.file.format)}${s.node >= 0 ? ` · ${m.label(s.node)}` : ""} · ${formatBytes(s.len)}`;
    if (s.node >= 0) {
      part.addEventListener("click", () => select(s.node));
      part.addEventListener("mouseenter", () => hover(s.node));
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
  const parts = [`${pct(share(Role.Content))} goes to the ${roleName(Role.Content, m.file.format).toLowerCase()}`];
  if (about > 0) parts.push(`${pct(about)} to information about it`);
  if (share(Role.Structure) >= 0.01) parts.push(`${pct(share(Role.Structure))} to the structure that holds it together`);
  if (share(Role.Hidden) > 0) parts.push(`${pct(share(Role.Hidden))} to bytes the format does not account for`);
  const thumb = share(Role.Thumbnail) > 0 ? " — a second, small copy of the picture among it" : "";
  const said = `Of its ${formatBytes(total)}, ${parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}` : parts[0]}${thumb}.`;
  group.append(el("p", "makeup-said", said), bar, legend);
  return group;
}
