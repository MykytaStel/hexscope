import { describe, expect, it } from "vitest";
import { check, DAILY, LIMITS } from "./feedback";
import { feedback, type Context, type D1Database, type D1Statement } from "./handlers";

const ok = { fetchSite: "same-origin", contentType: "application/json", length: 100 };

describe("what is kept of a message", () => {
  it("keeps the words, the kind, a way to answer and the report, within their limits", () => {
    const r = check({ ...ok, body: { kind: "bug", message: "  The PDF says it is damaged.\r\n", contact: "@me", report: "format pdf", page: "/uk/", lang: "uk", version: "0.3.0", device: "Safari on iPhone" } });
    expect(r).toEqual({ ok: true, feedback: { kind: "bug", message: "The PDF says it is damaged.", contact: "@me", report: "format pdf", page: "/uk/", lang: "uk", version: "0.3.0", device: "Safari on iPhone" } });
    const long = check({ ...ok, body: { kind: "idea", message: "x".repeat(10_000) } });
    expect(long.ok && long.feedback.message.length).toBe(LIMITS.message);
  });

  it("refuses what is not a person writing from this site", () => {
    expect(check({ ...ok, fetchSite: "cross-site", body: { kind: "bug", message: "hello" } })).toMatchObject({ ok: false, status: 403 });
    expect(check({ ...ok, contentType: "text/plain", body: { kind: "bug", message: "hello" } })).toMatchObject({ ok: false, status: 415 });
    expect(check({ ...ok, length: 70_000, body: { kind: "bug", message: "hello" } })).toMatchObject({ ok: false, status: 413 });
    expect(check({ ...ok, body: { kind: "bug", message: "hello", website: "http://spam" } })).toMatchObject({ ok: false, status: 400 });
    expect(check({ ...ok, body: { kind: "spam", message: "hello" } })).toMatchObject({ ok: false, status: 400 });
    expect(check({ ...ok, body: { kind: "bug", message: " " } })).toMatchObject({ ok: false, status: 400 });
  });

  it("drops what it cannot trust: odd pages, odd versions, control characters", () => {
    const r = check({ ...ok, body: { kind: "thanks", message: "thank\u0000 you", page: "https://evil", version: "<script>" } });
    expect(r).toMatchObject({ ok: true, feedback: { message: "thank you", page: "", version: "" } });
  });
});

/** A D1 database in memory: the rows kept, and how many today. */
function memoryDb(): D1Database & { rows: unknown[][] } {
  const rows: unknown[][] = [];
  const statement = (sql: string, values: unknown[] = []): D1Statement => ({
    bind: (...v) => statement(sql, v),
    async run() {
      if (sql.startsWith("INSERT")) rows.push(values);
      return {};
    },
    async first<T>() {
      return { n: rows.filter((r) => r[0] === values[0]).length } as T;
    },
  });
  return { rows, prepare: (sql) => statement(sql), batch: (s) => Promise.all(s.map((x) => x.run())) };
}

const post = (db: D1Database | undefined, body: unknown): Context => ({
  request: new Request("https://hexscope.org/api/feedback", {
    method: "POST",
    headers: { "Content-Type": "application/json", "Sec-Fetch-Site": "same-origin" },
    body: JSON.stringify(body),
  }),
  env: { COUNTS: db },
  params: {},
});

describe("the endpoint", () => {
  const now = new Date("2026-10-13T12:00:00Z");

  it("keeps a message and says no more than that it did", async () => {
    const db = memoryDb();
    const r = await feedback(post(db, { kind: "idea", message: "Please read RTF files" }), now);
    expect(r.status).toBe(204);
    expect(db.rows).toEqual([["2026-10-13", "idea", "Please read RTF files", "", "", "", "en", "", ""]]);
  });

  it("stops taking messages for the day after its limit", async () => {
    const db = memoryDb();
    for (let i = 0; i < DAILY; i++) db.rows.push(["2026-10-13"]);
    expect((await feedback(post(db, { kind: "bug", message: "one more" }), now)).status).toBe(429);
  });

  it("answers anything but a POST, and a missing database, plainly", async () => {
    const get: Context = { request: new Request("https://hexscope.org/api/feedback"), env: {}, params: {} };
    expect((await feedback(get, now)).status).toBe(405);
    expect((await feedback(post(undefined, { kind: "bug", message: "hello" }), now)).status).toBe(503);
  });
});
