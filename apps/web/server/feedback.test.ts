import { describe, expect, it } from "vitest";
import { check, DAILY, LIMITS, MAX_BODY_BYTES } from "./feedback";
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
      if (sql.startsWith("INSERT")) {
        if (rows.filter((r) => r[0] === values[0]).length >= Number(values[9])) return { meta: { changes: 0 } };
        rows.push(values.slice(0, 9));
        return { meta: { changes: 1 } };
      }
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

  it("enforces the last daily slot across concurrent requests", async () => {
    const db = memoryDb();
    for (let i = 0; i < DAILY - 1; i++) db.rows.push(["2026-10-13"]);
    const responses = await Promise.all(Array.from({ length: 8 }, () => feedback(post(db, { kind: "bug", message: "hello" }), now)));
    expect(responses.filter((r) => r.status === 204)).toHaveLength(1);
    expect(responses.filter((r) => r.status === 429)).toHaveLength(7);
    expect(db.rows).toHaveLength(DAILY);
  });

  it("answers anything but a POST, and a missing database, plainly", async () => {
    const get: Context = { request: new Request("https://hexscope.org/api/feedback"), env: {}, params: {} };
    expect((await feedback(get, now)).status).toBe(405);
    expect((await feedback(post(undefined, { kind: "bug", message: "hello" }), now)).status).toBe(503);
  });

  it("stops reading an oversized streamed body without a Content-Length", async () => {
    let cancelled = false;
    const stream = new ReadableStream<Uint8Array>({
      pull(controller) { controller.enqueue(new Uint8Array(MAX_BODY_BYTES + 1)); },
      cancel() { cancelled = true; },
    });
    const db = memoryDb();
    const request = new Request("https://hexscope.org/api/feedback", {
      method: "POST", headers: { "Content-Type": "application/json" }, body: stream, duplex: "half",
    } as RequestInit);
    expect((await feedback({ request, env: { COUNTS: db }, params: {} }, now)).status).toBe(413);
    expect(cancelled).toBe(true);
    expect(db.rows).toEqual([]);
  });

  it("counts UTF-8 bytes before parsing a body", async () => {
    const db = memoryDb();
    const ctx = post(db, { kind: "bug", message: "я".repeat(MAX_BODY_BYTES / 2) });
    expect((await feedback(ctx, now)).status).toBe(413);
    expect(db.rows).toEqual([]);
  });

  it("accepts a valid report whose UTF-8 bytes exceed the character limit", async () => {
    const db = memoryDb();
    const ctx = post(db, { kind: "bug", message: "hello", report: "я".repeat(40_000) });
    expect((await feedback(ctx, now)).status).toBe(204);
    expect(db.rows[0][4]).toHaveLength(40_000);
  });

  it("decodes a UTF-8 character split across stream chunks", async () => {
    const bytes = new TextEncoder().encode(JSON.stringify({ kind: "idea", message: "дякую" }));
    const split = bytes.indexOf(0xd0) + 1;
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(bytes.slice(0, split));
        controller.enqueue(bytes.slice(split));
        controller.close();
      },
    });
    const db = memoryDb();
    const request = new Request("https://hexscope.org/api/feedback", {
      method: "POST", headers: { "Content-Type": "application/json" }, body: stream, duplex: "half",
    } as RequestInit);
    expect((await feedback({ request, env: { COUNTS: db }, params: {} }, now)).status).toBe(204);
    expect(db.rows[0][2]).toBe("дякую");
  });
});
