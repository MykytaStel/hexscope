// How many people use hexscope, without knowing who any of them is.
//
// A page asks /api/seen/day, /api/seen/month and /api/seen/new, empty
// requests. Each answer tells the browser to keep it until its period ends —
// midnight UTC, the end of the month, a year — so the browser does not ask
// again within it, and the server hears from a browser at most once a day,
// once a month and once at all. Each answer also names its period's start as
// Last-Modified: a browser that asks again anyway (a hard reload) says so with
// If-Modified-Since and is not counted twice. What is kept is a total per day
// or month: no cookie, no address, no id, nothing about any file.

export type Period = "day" | "month" | "new";

export const PERIODS: readonly Period[] = ["day", "month", "new"];

/** What a day's first visit adds to besides "people": where it came from, kept as totals. */
export const BREAKDOWNS = ["country", "from", "page", "device", "lang"] as const;
export type Breakdown = (typeof BREAKDOWNS)[number];

export interface Visit {
  period: Period;
  now: Date;
  ifModifiedSince: string | null;
  userAgent: string;
  /** Sec-Fetch-Site: a browser's own fetch from this site says "same-origin". */
  fetchSite: string | null;
  country: string | null;
  page: string | null;
  device: string | null;
  lang: string | null;
  from: string | null;
}

export interface Row {
  period: string;
  metric: string;
  value: string;
}

export interface Answer {
  status: 200 | 304;
  /** The totals this visit adds one to; none for a repeat, a robot or another site. */
  rows: Row[];
  headers: Record<string, string>;
}

/** The site's own address: a visit from it is not "from" anywhere. */
export const HOST = "hexscope.pages.dev";

const DAY = 86_400_000;
/** "New" is counted against one fixed start: any earlier answer means seen before. */
const EVER = Date.UTC(2026, 0, 1);
const ROBOT = /bot|crawl|spider|slurp|headless|lighthouse|preview|externalhit|curl|wget|python|go-http|java\//i;

export function dayKey(at: Date | number): string {
  return new Date(at).toISOString().slice(0, 10);
}

export function monthKey(at: Date | number): string {
  return new Date(at).toISOString().slice(0, 7);
}

export function periodStart(period: Period, now: Date): number {
  if (period === "new") return EVER;
  const y = now.getUTCFullYear();
  const m = now.getUTCMonth();
  return period === "day" ? Date.UTC(y, m, now.getUTCDate()) : Date.UTC(y, m, 1);
}

export function periodEnd(period: Period, now: Date): number {
  if (period === "new") return now.getTime() + 365 * DAY;
  const start = periodStart(period, now);
  if (period === "day") return start + DAY;
  return Date.UTC(now.getUTCFullYear(), now.getUTCMonth() + 1, 1);
}

/** The answer to one visit: whether it counts, and how long the browser keeps it. */
export function answer(visit: Visit): Answer {
  const start = periodStart(visit.period, visit.now);
  const seconds = Math.max(60, Math.floor((periodEnd(visit.period, visit.now) - visit.now.getTime()) / 1000));
  const headers = {
    "Cache-Control": `private, max-age=${seconds}`,
    "Last-Modified": new Date(start).toUTCString(),
    "Content-Type": "text/plain; charset=utf-8",
    "X-Robots-Tag": "noindex",
  };
  const since = visit.ifModifiedSince ? Date.parse(visit.ifModifiedSince) : NaN;
  if (since >= start) return { status: 304, rows: [], headers };
  if (ROBOT.test(visit.userAgent) || (visit.fetchSite !== null && visit.fetchSite !== "same-origin")) {
    return { status: 200, rows: [], headers };
  }
  return { status: 200, rows: rowsFor(visit), headers };
}

function rowsFor(visit: Visit): Row[] {
  const day = dayKey(visit.now);
  if (visit.period === "month") return [{ period: monthKey(visit.now), metric: "people", value: "" }];
  if (visit.period === "new") return [{ period: day, metric: "new", value: "" }];
  const rows: Row[] = [{ period: day, metric: "people", value: "" }];
  const add = (metric: Breakdown, value: string | null) => {
    if (value) rows.push({ period: day, metric, value });
  };
  add("country", country(visit.country));
  add("from", fromHost(visit.from));
  add("page", page(visit.page));
  add("device", oneOf(visit.device, ["phone", "tablet", "computer"]));
  add("lang", oneOf(visit.lang, ["uk", "en"]));
  return rows;
}

function country(code: string | null): string {
  return code && /^[A-Z]{2}$/.test(code) && code !== "XX" && code !== "T1" ? code : "unknown";
}

/** A page's address without `.html` or `index`, or "other" for anything unexpected. */
export function page(path: string | null): string | null {
  if (!path) return null;
  const clean = path.replace(/\.html$/, "").replace(/\/index$/, "/");
  return /^\/[a-z0-9-]{0,64}$/.test(clean) ? clean : "other";
}

/** The site a visit came from, by name only, or nothing when it came from no site or this one. */
export function fromHost(host: string | null): string | null {
  if (!host) return null;
  const name = host.trim().toLowerCase().replace(/^www\./, "");
  if (!name || name === HOST || name.endsWith(`.${HOST}`)) return null;
  const label = "[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?";
  return name.length <= 100 && new RegExp(`^${label}(?:\\.${label})+$`).test(name) ? name : "other";
}

function oneOf(value: string | null, allowed: readonly string[]): string | null {
  return value && allowed.includes(value) ? value : null;
}

export interface Total {
  period: string;
  metric: string;
  value: string;
  n: number;
}

export interface Stats {
  updated: string;
  today: { day: string; people: number; new: number };
  month: { month: string; people: number };
  /** The last 30 days, oldest first, every day present. */
  days: { day: string; people: number; new: number }[];
  /** The last 12 months, oldest first, every month present. */
  months: { month: string; people: number }[];
  /** Over the last 30 days, largest first, at most 8 each. */
  top: Record<Breakdown, [string, number][]>;
}

export const DAYS = 30;
export const MONTHS = 12;
const TOP = 8;

export function firstDay(now: Date): string {
  return dayKey(periodStart("day", now) - (DAYS - 1) * DAY);
}

export function firstMonth(now: Date): string {
  return monthKey(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - (MONTHS - 1), 1));
}

/** The public figures, from the totals of the last 30 days and the monthly totals of the last 12 months. */
export function summarize(totals: readonly Total[], now: Date): Stats {
  const get = (period: string, metric: string) =>
    totals.find((t) => t.period === period && t.metric === metric && t.value === "")?.n ?? 0;
  const start = periodStart("day", now);
  const days = Array.from({ length: DAYS }, (_, i) => {
    const day = dayKey(start - (DAYS - 1 - i) * DAY);
    return { day, people: get(day, "people"), new: get(day, "new") };
  });
  const months = Array.from({ length: MONTHS }, (_, i) => {
    const month = monthKey(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - (MONTHS - 1 - i), 1));
    return { month, people: get(month, "people") };
  });
  const from = firstDay(now);
  const top = Object.fromEntries(
    BREAKDOWNS.map((metric) => {
      const sums = new Map<string, number>();
      for (const t of totals) {
        if (t.metric === metric && t.period.length === 10 && t.period >= from) sums.set(t.value, (sums.get(t.value) ?? 0) + t.n);
      }
      const ranked = [...sums].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, TOP);
      return [metric, ranked];
    }),
  ) as Record<Breakdown, [string, number][]>;
  const today = days[DAYS - 1];
  return { updated: now.toISOString(), today, month: months[MONTHS - 1], days, months, top };
}
