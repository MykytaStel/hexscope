// The two endpoints behind the counter, on Cloudflare Pages Functions with a
// D1 database bound as COUNTS (wrangler.toml). The database holds one table of
// totals (migrations/0001_counts.sql); nothing in it can name a visitor.

import { answer, firstDay, firstMonth, PERIODS, summarize, type Period, type Row, type Stats, type Total } from "./count";

export interface D1Statement {
  bind(...values: unknown[]): D1Statement;
  all<T>(): Promise<{ results: T[] }>;
  run(): Promise<unknown>;
}

export interface D1Database {
  prepare(query: string): D1Statement;
  batch(statements: D1Statement[]): Promise<unknown[]>;
}

export interface Env {
  COUNTS?: D1Database;
}

export interface Context {
  request: Request & { cf?: { country?: string } };
  env: Env;
  params: Record<string, string | string[] | undefined>;
}

const ADD = "INSERT INTO counts (period, metric, value, n) VALUES (?1, ?2, ?3, 1) ON CONFLICT (period, metric, value) DO UPDATE SET n = n + 1";

/** GET /api/seen/:period — counts a visit once per period, and says nothing back. */
export async function seen({ request, env, params }: Context, now = new Date()): Promise<Response> {
  const period = params.period;
  if (typeof period !== "string" || !PERIODS.includes(period as Period)) return new Response("Not found", { status: 404 });
  const header = (name: string) => request.headers.get(name);
  const reply = answer({
    period: period as Period,
    now,
    ifModifiedSince: header("If-Modified-Since"),
    userAgent: header("User-Agent") ?? "",
    fetchSite: header("Sec-Fetch-Site"),
    country: request.cf?.country ?? null,
    page: header("X-Seen-Page"),
    device: header("X-Seen-Device"),
    lang: header("X-Seen-Lang"),
    from: header("X-Seen-From"),
  });
  const headers = { ...reply.headers };
  if (reply.rows.length) {
    try {
      await add(env, reply.rows);
    } catch {
      // Not counted: the browser should ask again next time rather than keep this answer.
      headers["Cache-Control"] = "no-store";
    }
  }
  return new Response(reply.status === 304 ? null : "", { status: reply.status, headers });
}

async function add(env: Env, rows: readonly Row[]): Promise<void> {
  if (!env.COUNTS) throw new Error("No COUNTS database bound");
  const db = env.COUNTS;
  await db.batch(rows.map((r) => db.prepare(ADD).bind(r.period, r.metric, r.value)));
}

// The figures change slowly and the database bills each row it reads: one
// worker keeps what it last read for five minutes.
let kept: { at: number; stats: Stats } | null = null;
const KEEP = 5 * 60_000;

/** GET /api/stats — the public totals: people a day, a month, and where they came from. */
export async function stats({ env }: Context, now = new Date()): Promise<Response> {
  const headers = { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "public, max-age=300", "X-Robots-Tag": "noindex" };
  if (kept && now.getTime() - kept.at < KEEP) return new Response(JSON.stringify(kept.stats), { headers });
  if (!env.COUNTS) return new Response(JSON.stringify({ error: "unavailable" }), { status: 503, headers: { ...headers, "Cache-Control": "no-store" } });
  try {
    const db = env.COUNTS;
    const [days, months] = await Promise.all([
      db.prepare("SELECT period, metric, value, n FROM counts WHERE period >= ?1 AND length(period) = 10").bind(firstDay(now)).all<Total>(),
      db.prepare("SELECT period, metric, value, n FROM counts WHERE period >= ?1 AND length(period) = 7 AND metric = 'people'").bind(firstMonth(now)).all<Total>(),
    ]);
    const figures = summarize([...days.results, ...months.results], now);
    kept = { at: now.getTime(), stats: figures };
    return new Response(JSON.stringify(figures), { headers });
  } catch {
    return new Response(JSON.stringify({ error: "unavailable" }), { status: 503, headers: { ...headers, "Cache-Control": "no-store" } });
  }
}

/** For tests: forget what the last read kept. */
export function forgetKept(): void {
  kept = null;
}
