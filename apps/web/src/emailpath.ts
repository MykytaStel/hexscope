// The way an email came, drawn: who it says it is from, where it was sent
// from, the servers that passed it on, and you — with what should not be
// there marked on the way: replies that go elsewhere, a domain that did
// not vouch for it, links to other sites. All of it read from the headers
// the file already holds.
import type { FileModel } from "./model";

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Servers drawn at most; the rest are counted. */
const MAX_HOPS = 4;

/** The message's own headers: those beside its first From. */
function headers(m: FileModel): { name: string; value: string; node: number }[] {
  const labels = m.file.labels;
  const from = labels.findIndex((l) => l.toLowerCase() === "from");
  if (from < 0) return [];
  const path = m.path(from);
  const parent = path[path.length - 2] ?? 0;
  return Array.from(m.children(parent)).map((id) => ({ name: m.label(id).toLowerCase(), value: m.value(id), node: id }));
}

/** `Example Bank <security@example-bank.com>` → name and address. */
function address(value: string): { name: string; addr: string } {
  const angle = /<([^>]+)>/.exec(value);
  const addr = (angle ? angle[1] : value).trim();
  const name = angle ? value.slice(0, angle.index).replace(/"/g, "").trim() : "";
  return { name, addr };
}

/** The server that wrote a Received line: its `by`. */
function receivedBy(value: string): string {
  return /\bby\s+([^\s;()]+)/i.exec(value)?.[1] ?? "a server";
}

function fact(m: FileModel, kind: string): string | undefined {
  return m.file.facts.find((f) => f.kind === kind)?.text;
}

/** A stop on the way: what it is called, what it says, and whether it is a warning. */
function stop(label: string, main: string, notes: string[], tone: "" | "is-bad" | "is-good", node: number, select: (id: number) => void): HTMLElement {
  const box = el("li", `mail-stop ${tone}`);
  box.append(el("span", "mail-stop-label", label));
  const b = el("button", "mail-stop-main", main);
  b.title = "Show where in the file this is";
  if (node >= 0) b.addEventListener("click", () => select(node));
  else b.disabled = true;
  box.append(b);
  for (const n of notes) box.append(el("span", "mail-stop-note", n));
  return box;
}

/** The drawing, or null for a message with no sender to draw. */
export function emailPath(m: FileModel, select: (id: number) => void): HTMLElement | null {
  const hs = headers(m);
  const get = (name: string) => hs.find((h) => h.name === name);
  const fromH = get("from");
  if (!fromH) return null;
  const from = address(fromH.value);
  const figure = el("figure", "mail-path");
  const route = el("ol", "mail-route");
  route.setAttribute("aria-label", "The way this email came");

  // Who it says it is from, and whether that domain vouched for it.
  const failed = fact(m, "authfail");
  const passed = fact(m, "auth");
  const vouch = failed ? `✗ ${failed.split(":")[0]} — not vouched for` : passed ? `✓ ${passed.split(":")[0]}` : "Not checked by the servers";
  route.append(stop("Says it is from", from.addr, [...(from.name ? [from.name] : []), vouch], failed ? "is-bad" : passed ? "is-good" : "", fromH.node, select));

  // Where it really set out from.
  const sent = fact(m, "sentfrom");
  if (sent) {
    const ip = sent.split(",")[0];
    const notes = [fact(m, "computer"), fact(m, "mailer"), fact(m, "timezone")?.split(",")[0]].filter((x): x is string => !!x);
    const node = m.file.facts.find((f) => f.kind === "sentfrom")?.node ?? -1;
    route.append(stop("Sent from", ip, notes, "", node, select));
  }

  // The servers, in the order they passed it on: the last Received line is the first server.
  const hops = hs.filter((h) => h.name === "received").reverse();
  for (const h of hops.slice(0, MAX_HOPS)) route.append(stop("Passed through", receivedBy(h.value), [], "", h.node, select));
  if (hops.length > MAX_HOPS) route.append(el("li", "mail-stop is-more", `and ${hops.length - MAX_HOPS} more servers`));

  const to = get("to");
  route.append(stop("To", to ? address(to.value).addr : "you", [], "", to?.node ?? -1, select));
  figure.append(route);

  // What leaves the way: where a reply, a bounce or a click would go.
  const off = el("ul", "mail-off");
  const replyTo = get("reply-to");
  if (fact(m, "replyto") && replyTo) {
    off.append(stop("Replying goes to", address(replyTo.value).addr, ["another domain than the sender's"], "is-bad", replyTo.node, select));
  }
  const links = m.file.facts.find((f) => f.kind === "linkmismatch" || f.kind === "linkidn");
  if (links) {
    const where = /goes to (\S+)$/.exec(fact(m, "linkmismatch") ?? "")?.[1] ?? fact(m, "linkidn")?.split(",")[0] ?? "";
    off.append(stop("A link goes to", where, ["not the site its words name"], "is-bad", links.node, select));
  }
  const risky = m.file.facts.find((f) => f.kind === "riskyfile");
  if (risky) {
    const name = /“([^”]+)”/.exec(risky.text)?.[1] ?? "an attachment";
    off.append(stop("Attached", name, ["runs, or opens a site, when opened"], "is-bad", risky.node, select));
  }
  if (off.childElementCount > 0) figure.append(off);
  figure.append(el("figcaption", undefined, "Read from the message's own headers: nothing was looked up or visited."));
  return figure;
}
