// The endpoint behind the counter, on Cloudflare Pages Functions with a
// D1 database bound as COUNTS (wrangler.toml). The database holds one table of
// totals (migrations/0001_counts.sql); nothing in it can name a visitor. The
// totals are read only by the account's owner: \`pnpm stats\` (scripts/stats.mjs).

import { answer, PERIODS, type Period, type Row } from "./count";

export interface D1Statement {
  bind(...values: unknown[]): D1Statement;
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
