// The way an email came, drawn: who it says it is from, where it was sent
// from, the servers that passed it on, and you — with what should not be
// there marked on the way: replies that go elsewhere, a domain that did
// not vouch for it, links to other sites. All of it read from the headers
// the file already holds. `mailRoute` reads it; `emailPath` draws it.
import type { FileModel } from "./model";

/** Servers drawn at most; the rest are counted. */
const MAX_HOPS = 4;

/** A stop on the way, or a way off it. */
export interface Stop {
  label: string;
  main: string;
  notes: string[];
  tone: "" | "bad" | "good";
  /** The part of the file it comes from, or -1. */
  node: number;
}

export interface Route {
  stops: Stop[];
  /** Servers past those in `stops`. */
  moreHops: number;
  /** Where a reply, a click or an attachment leads off the way. */
  off: Stop[];
}

/** The message's own headers: those beside its first From. */
function headers(m: FileModel): { name: string; value: string; node: number }[] {
  const from = m.file.labels.findIndex((l) => l.toLowerCase() === "from");
  if (from < 0) return [];
  const path = m.path(from);
  const parent = path[path.length - 2] ?? 0;
  return Array.from(m.children(parent)).map((id) => ({ name: m.label(id).toLowerCase(), value: m.value(id), node: id }));
}

/** `Example Bank <security@example-bank.com>` → name and address. */
export function address(value: string): { name: string; addr: string } {
  const angle = /<([^>]+)>/.exec(value);
  const addr = (angle ? angle[1] : value).trim();
  const name = angle ? value.slice(0, angle.index).replace(/"/g, "").trim() : "";
  return { name, addr };
}

/** The server that wrote a Received line: its `by`. */
export function receivedBy(value: string): string {
  return /\bby\s+([^\s;()]+)/i.exec(value)?.[1] ?? "a server";
}

/** The way, read from the headers and the facts; null for a message with no sender. */
export function mailRoute(m: FileModel): Route | null {
  const hs = headers(m);
  const get = (name: string) => hs.find((h) => h.name === name);
  const fact = (kind: string) => m.file.facts.find((f) => f.kind === kind);
  const fromH = get("from");
  if (!fromH) return null;
  const from = address(fromH.value);
  const stops: Stop[] = [];

  // Who it says it is from, and whether that domain vouched for it.
  const failed = fact("authfail")?.text;
  const passed = fact("auth")?.text;
  const vouch = failed ? `✗ ${failed.split(":")[0]} — not vouched for` : passed ? `✓ ${passed.split(":")[0]}` : "Not checked by the servers";
  stops.push({
    label: "Says it is from",
    main: from.addr,
    notes: [...(from.name ? [from.name] : []), vouch],
    tone: failed ? "bad" : passed ? "good" : "",
    node: fromH.node,
  });

  // Where it really set out from.
  const sent = fact("sentfrom");
  if (sent) {
    const notes = [fact("computer")?.text, fact("mailer")?.text, fact("timezone")?.text.split(",")[0]].filter((x): x is string => !!x);
    stops.push({ label: "Sent from", main: sent.text.split(",")[0], notes, tone: "", node: sent.node });
  }

  // The servers in the order they passed it on: the last Received line is the first server.
  const hops = hs.filter((h) => h.name === "received").reverse();
  for (const h of hops.slice(0, MAX_HOPS)) stops.push({ label: "Passed through", main: receivedBy(h.value), notes: [], tone: "", node: h.node });

  const to = get("to");
  stops.push({ label: "To", main: to ? address(to.value).addr : "you", notes: [], tone: "", node: to?.node ?? -1 });

  const off: Stop[] = [];
  const replyTo = get("reply-to");
  if (fact("replyto") && replyTo) {
    off.push({ label: "Replying goes to", main: address(replyTo.value).addr, notes: ["another domain than the sender's"], tone: "bad", node: replyTo.node });
  }
  const mismatch = fact("linkmismatch");
  const lookalike = fact("linkidn");
  const link = mismatch ?? lookalike;
  if (link) {
    const where = (mismatch && /goes to (\S+)$/.exec(mismatch.text)?.[1]) || lookalike?.text.split(",")[0] || "";
    off.push({ label: "A link goes to", main: where, notes: ["not the site its words name"], tone: "bad", node: link.node });
  }
  const risky = fact("riskyfile");
  if (risky) {
    off.push({
      label: "Attached",
      main: /“([^”]+)”/.exec(risky.text)?.[1] ?? "an attachment",
      notes: ["runs, or opens a site, when opened"],
      tone: "bad",
      node: risky.node,
    });
  }
  return { stops, moreHops: Math.max(0, hops.length - MAX_HOPS), off };
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function drawStop(s: Stop, select: (id: number) => void): HTMLElement {
  const box = el("li", `mail-stop${s.tone ? ` is-${s.tone}` : ""}`);
  box.append(el("span", "mail-stop-label", s.label));
  const b = el("button", "mail-stop-main", s.main);
  b.title = "Show where in the file this is";
  if (s.node >= 0) b.addEventListener("click", () => select(s.node));
  else b.disabled = true;
  box.append(b);
  for (const n of s.notes) box.append(el("span", "mail-stop-note", n));
  return box;
}

/** The drawing, or null for a message with no sender to draw. */
export function emailPath(m: FileModel, select: (id: number) => void): HTMLElement | null {
  const r = mailRoute(m);
  if (!r) return null;
  const figure = el("figure", "mail-path");
  const route = el("ol", "mail-route");
  route.setAttribute("aria-label", "The way this email came");
  const last = r.stops.length - 1;
  r.stops.forEach((s, i) => {
    // The servers not drawn are counted just before the recipient.
    if (i === last && r.moreHops > 0) route.append(el("li", "mail-stop is-more", `and ${r.moreHops} more servers`));
    route.append(drawStop(s, select));
  });
  figure.append(route);
  if (r.off.length > 0) {
    const off = el("ul", "mail-off");
    off.append(...r.off.map((s) => drawStop(s, select)));
    figure.append(off);
  }
  figure.append(el("figcaption", undefined, "Read from the message's own headers: nothing was looked up or visited."));
  return figure;
}
