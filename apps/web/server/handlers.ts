// The endpoints behind the counter and the messages people send, on
// Cloudflare Pages Functions with a D1 database bound as COUNTS
// (wrangler.toml). It holds a table of totals (migrations/0001_counts.sql),
// in which nothing can name a visitor, and the messages people chose to send
// (migrations/0002_feedback.sql). Both are read only by the account's owner:
// \`pnpm stats\` (scripts/stats.mjs) and \`pnpm feedback\` (scripts/feedback.mjs).

import { answer, PERIODS, type Period, type Row } from "./count";
import { check, checkHeaders, DAILY, MAX_BODY_BYTES, type Feedback } from "./feedback";

export interface D1Statement {
  bind(...values: unknown[]): D1Statement;
  run(): Promise<unknown>;
  first<T = Record<string, unknown>>(): Promise<T | null>;
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

/** The messages table, made on first use so a new deployment needs no step of its own. */
const FEEDBACK_TABLE = `CREATE TABLE IF NOT EXISTS feedback (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  day TEXT NOT NULL,
  kind TEXT NOT NULL,
  message TEXT NOT NULL,
  contact TEXT NOT NULL DEFAULT '',
  report TEXT NOT NULL DEFAULT '',
  page TEXT NOT NULL DEFAULT '',
  lang TEXT NOT NULL DEFAULT '',
  version TEXT NOT NULL DEFAULT '',
  device TEXT NOT NULL DEFAULT '',
  done INTEGER NOT NULL DEFAULT 0
)`;
// The limit and insert must share one SQLite statement: concurrent requests
// must not all observe the same remaining slot.
const KEEP = "INSERT INTO feedback (day, kind, message, contact, report, page, lang, version, device) SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9 WHERE (SELECT COUNT(*) FROM feedback WHERE day = ?1) < ?10";

/** POST /api/feedback — keeps a message someone chose to send; says only whether it was kept. */
export async function feedback({ request, env }: Context, now = new Date()): Promise<Response> {
  if (request.method !== "POST") return new Response("POST only", { status: 405, headers: { Allow: "POST" } });
  // Reject invalid origins and media types before consuming an untrusted body.
  const headers = { fetchSite: request.headers.get("Sec-Fetch-Site"), contentType: request.headers.get("Content-Type") };
  const invalid = checkHeaders(headers.fetchSite, headers.contentType);
  if (invalid) return new Response(invalid.reason, { status: invalid.status });
  const declared = request.headers.get("Content-Length");
  if (declared && /^\d+$/.test(declared) && Number(declared) > MAX_BODY_BYTES) return new Response("too large", { status: 413 });
  const reader = request.body?.getReader();
  const decoder = new TextDecoder();
  let raw = "";
  let length = 0;
  if (reader) {
    try {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        length += value.byteLength;
        if (length > MAX_BODY_BYTES) {
          void reader.cancel().catch(() => {});
          return new Response("too large", { status: 413 });
        }
        raw += decoder.decode(value, { stream: true });
      }
      raw += decoder.decode();
    } catch {
      return new Response("invalid body", { status: 400 });
    } finally {
      reader.releaseLock();
    }
  }
  let body: unknown = null;
  try {
    body = JSON.parse(raw);
  } catch {
    // Checked below as nothing said.
  }
  const checked = check({
    ...headers,
    length: raw.length,
    body,
  });
  if (!checked.ok) return new Response(checked.reason, { status: checked.status });
  if (!env.COUNTS) return new Response("not kept: no database", { status: 503 });
  try {
    const kept = await keep(env.COUNTS, checked.feedback, now.toISOString().slice(0, 10));
    return kept ? new Response(null, { status: 204 }) : new Response("too many messages today", { status: 429 });
  } catch {
    return new Response("not kept", { status: 503 });
  }
}

async function keep(db: D1Database, f: Feedback, day: string): Promise<boolean> {
  await db.prepare(FEEDBACK_TABLE).run();
  const result = await db.prepare(KEEP).bind(day, f.kind, f.message, f.contact, f.report, f.page, f.lang, f.version, f.device, DAILY).run();
  return (result as { meta?: { changes?: number } }).meta?.changes === 1;
}
