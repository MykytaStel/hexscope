// The /stats page: every number the visit counter keeps, drawn from
// /api/stats. Text is written in English and translated like any page's.
import "./stats.css";
import { currentLocale } from "./i18n";

interface Stats {
  today: { day: string; people: number; new: number };
  month: { month: string; people: number };
  days: { day: string; people: number; new: number }[];
  months: { month: string; people: number }[];
  top: Record<"country" | "from" | "page" | "device" | "lang", [string, number][]>;
}

const root = document.getElementById("stats");
let figures: Stats | null = null;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className = "", text = ""): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

/** "1 person", "5 people"; in Ukrainian by its three forms: 1 людина, 3 людини, 5 людей. */
function plural(n: number, en: [string, string], uk: [string, string, string]): string {
  if (currentLocale() !== "uk") return n === 1 ? en[0] : en[1];
  const form = new Intl.PluralRules("uk").select(n);
  return form === "one" ? uk[0] : form === "few" ? uk[1] : uk[2];
}

const number = (n: number) => n.toLocaleString(currentLocale() === "uk" ? "uk-UA" : "en-GB");

function dayLabel(day: string, style: "short" | "long" = "short"): string {
  return new Intl.DateTimeFormat(currentLocale(), { day: "numeric", month: style === "short" ? "short" : "long", timeZone: "UTC" }).format(new Date(`${day}T00:00:00Z`));
}

function monthLabel(month: string): string {
  return new Intl.DateTimeFormat(currentLocale(), { month: "short", year: "2-digit", timeZone: "UTC" }).format(new Date(`${month}-01T00:00:00Z`));
}

/** A clean top for the axis: 1, 2 or 5 times a power of ten, at least the largest value. */
function niceTop(max: number): number {
  if (max <= 4) return 4;
  const power = 10 ** Math.floor(Math.log10(max));
  return [1, 2, 5, 10].map((m) => m * power).find((t) => t >= max) ?? 10 * power;
}

function tile(label: string, value: number, note = ""): HTMLElement {
  const box = el("div", "stats-tile");
  box.append(el("span", "stats-tile-label", label), el("strong", "stats-tile-value", number(value)));
  if (note) box.append(el("span", "stats-tile-note", note));
  return box;
}

interface Column {
  key: string;
  label: string;
  value: number;
  tip: string;
}

/** Columns from one baseline, one series; a tooltip on hover, a table for everything. */
function columns(title: string, cols: Column[], caption: (c: Column) => string): HTMLElement {
  const figure = el("figure", "stats-chart");
  figure.append(el("figcaption", "stats-chart-title", title));
  const top = niceTop(Math.max(0, ...cols.map((c) => c.value)));
  const plot = el("div", "stats-plot");
  plot.setAttribute("aria-hidden", "true");
  for (const t of [top, top / 2, 0]) {
    const line = el("div", "stats-grid");
    line.style.bottom = `${(t / top) * 100}%`;
    line.append(el("span", "stats-grid-label", number(t)));
    plot.append(line);
  }
  const bars = el("div", "stats-bars");
  const tip = el("div", "stats-tip");
  cols.forEach((c, i) => {
    const slot = el("div", "stats-slot");
    const bar = el("div", "stats-bar");
    bar.style.height = c.value ? `max(2px, ${(c.value / top) * 100}%)` : "0";
    slot.append(bar);
    if (i === cols.length - 1 && c.value) {
      const cap = el("span", "stats-cap", number(c.value));
      cap.style.bottom = `${(c.value / top) * 100}%`;
      slot.append(cap);
    }
    slot.addEventListener("pointerenter", () => {
      tip.textContent = c.tip;
      tip.hidden = false;
      const left = (i + 0.5) / cols.length;
      tip.style.left = `${left * 100}%`;
      tip.dataset.edge = left < 0.2 ? "start" : left > 0.8 ? "end" : "";
    });
    bars.append(slot);
  });
  bars.addEventListener("pointerleave", () => (tip.hidden = true));
  tip.hidden = true;
  plot.append(bars, tip);
  const axis = el("div", "stats-axis");
  axis.setAttribute("aria-hidden", "true");
  for (const i of [0, Math.floor((cols.length - 1) / 2), cols.length - 1]) axis.append(el("span", "", cols[i].label));
  figure.append(plot, axis, table(cols, caption));
  return figure;
}

function table(cols: Column[], caption: (c: Column) => string): HTMLElement {
  const details = el("details", "stats-table");
  details.append(el("summary", "", "Show as a table"));
  const t = el("table");
  for (const c of cols) {
    const row = el("tr");
    row.append(el("th", "", c.key), el("td", "", caption(c)));
    t.append(row);
  }
  details.append(t);
  return details;
}

const NAMES: Record<string, string> = {
  phone: "Phone",
  tablet: "Tablet",
  computer: "Computer",
  uk: "Ukrainian",
  en: "English",
  unknown: "Unknown",
  other: "Other",
};

function nameOf(metric: string, value: string): string {
  if (NAMES[value]) return NAMES[value];
  if (metric === "country") {
    try {
      return new Intl.DisplayNames([currentLocale()], { type: "region" }).of(value) ?? value;
    } catch {
      return value;
    }
  }
  return value;
}

/** Largest first, as bars against the largest. */
function ranked(title: string, metric: keyof Stats["top"], rows: [string, number][]): HTMLElement {
  const box = el("section", "stats-ranked");
  box.append(el("h3", "", title));
  if (!rows.length) {
    box.append(el("p", "stats-none", "Nothing yet."));
    return box;
  }
  const max = rows[0][1];
  const list = el("ol");
  for (const [value, n] of rows) {
    const item = el("li");
    const name = el("span", metric === "page" || metric === "from" ? "stats-name stats-name-code" : "stats-name", nameOf(metric, value));
    const bar = el("span", "stats-rank-bar");
    bar.style.width = `${(n / max) * 100}%`;
    item.append(name, el("span", "stats-rank-value", number(n)), el("span", "stats-rank-track"));
    item.lastElementChild?.append(bar);
    list.append(item);
  }
  box.append(list);
  return box;
}

function render(): void {
  if (!root || !figures) return;
  const s = figures;
  const people = (n: number) => `${number(n)} ${plural(n, ["person", "people"], ["людина", "людини", "людей"])}`;
  const newOf = (n: number) => (currentLocale() === "uk" ? `нових — ${number(n)}` : `${number(n)} new`);
  const tiles = el("div", "stats-tiles");
  tiles.append(
    tile("Today", s.today.people, s.today.new ? `${newOf(s.today.new)}` : ""),
    tile("This month", s.month.people),
    tile("Last 30 days", s.days.reduce((sum, d) => sum + d.people, 0), "a person counted once a day"),
  );
  const daily = columns(
    "People a day, last 30 days",
    s.days.map((d) => ({ key: dayLabel(d.day, "long"), label: dayLabel(d.day), value: d.people, tip: `${dayLabel(d.day, "long")}: ${people(d.people)}, ${newOf(d.new)}` })),
    (c) => c.tip.slice(c.key.length + 2),
  );
  const monthly = columns(
    "People a month",
    s.months.map((m) => ({ key: monthLabel(m.month), label: monthLabel(m.month), value: m.people, tip: `${monthLabel(m.month)}: ${people(m.people)}` })),
    (c) => people(c.value),
  );
  const where = el("section", "stats-where");
  const breakdowns = el("div", "stats-breakdowns");
  where.append(el("h2", "", "Where visits come from"), breakdowns);
  breakdowns.append(
    ranked("Countries", "country", s.top.country),
    ranked("Came from", "from", s.top.from),
    ranked("First page", "page", s.top.page),
    ranked("Device", "device", s.top.device),
    ranked("Language", "lang", s.top.lang),
  );
  const note = el("p", "stats-updated", "Breakdowns cover a person's first visit each day, over the last 30 days. Days are in UTC.");
  root.replaceChildren(tiles, daily, monthly, where, note);
}

async function load(): Promise<void> {
  if (!root) return;
  try {
    const res = await fetch("/api/stats", { credentials: "omit" });
    if (!res.ok) throw new Error(String(res.status));
    figures = (await res.json()) as Stats;
    render();
  } catch {
    root.replaceChildren(el("p", "stats-none", "The numbers are not available right now."));
  }
}

window.addEventListener("hexscope:locale", render);
void load();
