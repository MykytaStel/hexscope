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

/** The site's own addresses, old and new: a visit from them is not "from" anywhere. */
export const HOSTS = ["hexscope.org", "hexscope.pages.dev"];

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
  if (!name || HOSTS.some((host) => name === host || name.endsWith(`.${host}`))) return null;
  const label = "[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?";
  return name.length <= 100 && new RegExp(`^${label}(?:\\.${label})+$`).test(name) ? name : "other";
}

function oneOf(value: string | null, allowed: readonly string[]): string | null {
  return value && allowed.includes(value) ? value : null;
}
