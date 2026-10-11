import { beforeEach, describe, expect, it } from "vitest";
import { answer, fromHost, page, periodEnd, periodStart, summarize, type Total, type Visit } from "./count";
import { forgetKept, seen, stats, type Context, type D1Database, type D1Statement } from "./handlers";

const at = (iso: string) => new Date(iso);

const visit = (over: Partial<Visit> = {}): Visit => ({
  period: "day",
  now: at("2026-10-10T21:00:00Z"),
  ifModifiedSince: null,
  userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) AppleWebKit/605.1.15 Safari/604.1",
  fetchSite: "same-origin",
  country: "UA",
  page: "/",
  device: "phone",
  lang: "uk",
  from: null,
  ...over,
});

describe("counting a visit once per period", () => {
  it("keeps the day's answer until midnight UTC and the month's until the month ends", () => {
    expect(answer(visit()).headers["Cache-Control"]).toBe("private, max-age=10800");
    expect(answer(visit({ period: "month" })).headers["Cache-Control"]).toBe(`private, max-age=${21 * 86400 + 3 * 3600}`);
    expect(periodEnd("month", at("2026-12-31T23:00:00Z"))).toBe(Date.UTC(2027, 0, 1));
    expect(answer(visit({ period: "new" })).headers["Cache-Control"]).toBe(`private, max-age=${365 * 86400}`);
  });

  it("names the period's start, so a browser asking again within it is not counted twice", () => {
    const first = answer(visit());
    expect(first.status).toBe(200);
    expect(first.headers["Last-Modified"]).toBe("Sat, 10 Oct 2026 00:00:00 GMT");
    const again = answer(visit({ ifModifiedSince: first.headers["Last-Modified"] }));
    expect(again.status).toBe(304);
    expect(again.rows).toEqual([]);
  });

  it("counts a browser again once its answer is from an earlier day or month", () => {
    const yesterday = new Date(periodStart("day", at("2026-10-09T12:00:00Z"))).toUTCString();
    expect(answer(visit({ ifModifiedSince: yesterday })).rows[0]).toEqual({ period: "2026-10-10", metric: "people", value: "" });
    const lastMonth = new Date(periodStart("month", at("2026-09-30T12:00:00Z"))).toUTCString();
    expect(answer(visit({ period: "month", ifModifiedSince: lastMonth })).rows).toEqual([{ period: "2026-10", metric: "people", value: "" }]);
  });

  it("counts a browser as new only when it has never had an answer", () => {
    expect(answer(visit({ period: "new" })).rows).toEqual([{ period: "2026-10-10", metric: "new", value: "" }]);
    expect(answer(visit({ period: "new", ifModifiedSince: "Thu, 01 Jan 2026 00:00:00 GMT" })).status).toBe(304);
  });

  it("does not count robots or requests from other sites", () => {
    expect(answer(visit({ userAgent: "Mozilla/5.0 (compatible; Googlebot/2.1)" })).rows).toEqual([]);
    expect(answer(visit({ userAgent: "HeadlessChrome/140" })).rows).toEqual([]);
    expect(answer(visit({ fetchSite: "cross-site" })).rows).toEqual([]);
  });

  it("adds a day's first visit to totals of country, source, page, device and language", () => {
    const rows = answer(visit({ from: "News.YCombinator.com" })).rows.map((r) => `${r.metric}=${r.value}`);
    expect(rows).toEqual(["people=", "country=UA", "from=news.ycombinator.com", "page=/", "device=phone", "lang=uk"]);
  });

  it("keeps anything unexpected out of the totals", () => {
    const rows = answer(visit({ country: "XX", page: "/<script>", device: "fridge", lang: "fr" })).rows.map((r) => `${r.metric}=${r.value}`);
    expect(rows).toEqual(["people=", "country=unknown", "page=other"]);
  });
});

describe("what a visit is said to come from", () => {
  it("is a site's name, without www, and never this site", () => {
    expect(fromHost("www.reddit.com")).toBe("reddit.com");
    expect(fromHost("hexscope.pages.dev")).toBeNull();
    expect(fromHost("feat-x.hexscope.pages.dev")).toBeNull();
    expect(fromHost("")).toBeNull();
    expect(fromHost("not a host")).toBe("other");
    expect(fromHost(`${"a".repeat(98)}.io`)).toBe("other");
  });

  it("names pages by the address the host serves them at", () => {
    expect(page("/remove-location-from-photo.html")).toBe("/remove-location-from-photo");
    expect(page("/index.html")).toBe("/");
    expect(page("/stats")).toBe("/stats");
    expect(page("/a/b")).toBe("other");
  });
});

describe("the public figures", () => {
  const now = at("2026-10-10T12:00:00Z");
  const totals: Total[] = [
    { period: "2026-10-10", metric: "people", value: "", n: 12 },
    { period: "2026-10-10", metric: "new", value: "", n: 9 },
    { period: "2026-10-09", metric: "people", value: "", n: 5 },
    { period: "2026-09-01", metric: "people", value: "", n: 99 },
    { period: "2026-10", metric: "people", value: "", n: 15 },
    { period: "2026-09", metric: "people", value: "", n: 40 },
    { period: "2026-10-10", metric: "country", value: "UA", n: 8 },
    { period: "2026-10-09", metric: "country", value: "UA", n: 3 },
    { period: "2026-10-10", metric: "country", value: "PL", n: 4 },
    { period: "2026-09-01", metric: "country", value: "US", n: 99 },
  ];

  it("fills every one of the last 30 days and 12 months, oldest first", () => {
    const s = summarize(totals, now);
    expect(s.days).toHaveLength(30);
    expect(s.days[0]).toEqual({ day: "2026-09-11", people: 0, new: 0 });
    expect(s.days.at(-2)).toEqual({ day: "2026-10-09", people: 5, new: 0 });
    expect(s.today).toEqual({ day: "2026-10-10", people: 12, new: 9 });
    expect(s.months).toHaveLength(12);
    expect(s.months[0].month).toBe("2025-11");
    expect(s.months.slice(-2)).toEqual([{ month: "2026-09", people: 40 }, { month: "2026-10", people: 15 }]);
    expect(s.month).toEqual({ month: "2026-10", people: 15 });
  });

  it("ranks where people came from over the last 30 days only", () => {
    expect(summarize(totals, now).top.country).toEqual([["UA", 11], ["PL", 4]]);
    expect(summarize(totals, now).top.from).toEqual([]);
  });
});

/** A D1 database in memory, enough for the two statements the endpoints run. */
function memoryDb(fail = false): D1Database & { totals: Map<string, number> } {
  const totals = new Map<string, number>();
  const statement = (sql: string, values: unknown[] = []): D1Statement => ({
    bind: (...v) => statement(sql, v),
    async run() {
      if (fail) throw new Error("D1 is down");
      const key = values.join("|");
      totals.set(key, (totals.get(key) ?? 0) + 1);
      return {};
    },
    async all<T>() {
      if (fail) throw new Error("D1 is down");
      const from = String(values[0]);
      const results = [...totals].map(([k, n]) => {
        const [period, metric, value] = k.split("|");
        return { period, metric, value, n };
      });
      const wanted = sql.includes("length(period) = 10")
        ? results.filter((r) => r.period.length === 10 && r.period >= from)
        : results.filter((r) => r.period.length === 7 && r.period >= from && r.metric === "people");
      return { results: wanted as T[] };
    },
  });
  return {
    totals,
    prepare: (sql) => statement(sql),
    batch: (statements) => Promise.all(statements.map((s) => s.run())),
  };
}

function context(db: D1Database | undefined, path: string, headers: Record<string, string> = {}, key?: string): Context {
  const period = path.split("/").pop();
  return {
    request: Object.assign(new Request(`https://hexscope.pages.dev${path}`, { headers: { "Sec-Fetch-Site": "same-origin", ...headers } }), {
      cf: { country: "UA" },
    }),
    env: { COUNTS: db, STATS_KEY: key },
    params: { period },
  };
}

describe("the endpoints", () => {
  beforeEach(forgetKept);
  const now = at("2026-10-10T12:00:00Z");

  it("counts a visit and its details, then nothing for the same browser that day", async () => {
    const db = memoryDb();
    const first = await seen(context(db, "/api/seen/day", { "X-Seen-Page": "/", "X-Seen-Lang": "en" }), now);
    expect(first.status).toBe(200);
    expect(first.headers.get("Cache-Control")).toBe("private, max-age=43200");
    expect(Object.fromEntries(db.totals)).toEqual({ "2026-10-10|people|": 1, "2026-10-10|country|UA": 1, "2026-10-10|page|/": 1, "2026-10-10|lang|en": 1 });
    const again = await seen(context(db, "/api/seen/day", { "If-Modified-Since": first.headers.get("Last-Modified") ?? "" }), now);
    expect(again.status).toBe(304);
    expect(db.totals.get("2026-10-10|people|")).toBe(1);
  });

  it("answers an unknown period with 404 and counts nothing", async () => {
    const db = memoryDb();
    expect((await seen(context(db, "/api/seen/week"), now)).status).toBe(404);
    expect(db.totals.size).toBe(0);
  });

  it("asks the browser not to keep an answer that was not counted", async () => {
    const res = await seen(context(memoryDb(true), "/api/seen/day"), now);
    expect(res.status).toBe(200);
    expect(res.headers.get("Cache-Control")).toBe("no-store");
  });

  it("serves the figures from the totals", async () => {
    const db = memoryDb();
    await seen(context(db, "/api/seen/day"), now);
    await seen(context(db, "/api/seen/month"), now);
    await seen(context(db, "/api/seen/new"), now);
    const res = await stats(context(db, "/api/stats", { Authorization: "Bearer the-key" }, "the-key"), now);
    expect(res.headers.get("Cache-Control")).toBe("private, no-store");
    const body = await res.json();
    expect(body.today).toEqual({ day: "2026-10-10", people: 1, new: 1 });
    expect(body.month).toEqual({ month: "2026-10", people: 1 });
    expect(body.top.country).toEqual([["UA", 1]]);
  });

  it("shows the figures only to whoever has the key", async () => {
    const db = memoryDb();
    expect((await stats(context(db, "/api/stats"), now)).status).toBe(404);
    expect((await stats(context(db, "/api/stats", {}, "the-key"), now)).status).toBe(401);
    expect((await stats(context(db, "/api/stats", { Authorization: "Bearer the-kez" }, "the-key"), now)).status).toBe(401);
    expect((await stats(context(db, "/api/stats", { Authorization: "Bearer the-key" }, "the-key"), now)).status).toBe(200);
  });

  it("says the figures are unavailable without a database", async () => {
    const auth = { Authorization: "Bearer k" };
    expect((await stats(context(undefined, "/api/stats", auth, "k"), now)).status).toBe(503);
    expect((await stats(context(memoryDb(true), "/api/stats", auth, "k"), now)).status).toBe(503);
  });
});
