// The visit counter's numbers, read straight from its database with your
// Cloudflare login: only the account's owner can run it, and no key is needed.
//
//   pnpm stats
import { execFileSync } from "node:child_process";
import { join } from "node:path";

const web = join(new URL("..", import.meta.url).pathname, "apps/web");
const day = (offset = 0) => new Date(Date.now() - offset * 86_400_000).toISOString().slice(0, 10);
const month = day().slice(0, 7);
const since = day(29);

const sql = `SELECT period, metric, value, n FROM counts WHERE period >= '${since}' OR (length(period) = 7 AND period >= '${since.slice(0, 7)}')`;
const out = execFileSync(
  "npx",
  ["--yes", "wrangler@4", "d1", "execute", "hexscope-counts", "--remote", "--json", "--command", sql],
  { cwd: web, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
);
const rows = JSON.parse(out.slice(out.indexOf("[")))[0].results;
const total = (period, metric) => rows.find((r) => r.period === period && r.metric === metric && r.value === "")?.n ?? 0;
const top = (metric) => {
  const sums = new Map();
  for (const r of rows) if (r.metric === metric && r.period.length === 10) sums.set(r.value, (sums.get(r.value) ?? 0) + r.n);
  return [...sums].sort((a, b) => b[1] - a[1]).slice(0, 5).map(([v, n]) => `${v || "—"} ${n}`).join(", ") || "—";
};

const days = Array.from({ length: 30 }, (_, i) => day(29 - i));
const people = days.map((d) => total(d, "people"));
const bars = " ▁▂▃▄▅▆▇█";
const max = Math.max(1, ...people);

console.log(`
hexscope — people (counted without knowing who)

  today        ${total(day(), "people")}  (new: ${total(day(), "new")})
  yesterday    ${total(day(1), "people")}
  this month   ${total(month, "people")}
  last 30 days ${people.reduce((a, b) => a + b, 0)} person-days

  ${days[0].slice(5)} ${people.map((n) => bars[Math.round((n / max) * 8)]).join("")} ${days[29].slice(5)}

  countries  ${top("country")}
  came from  ${top("from")}
  first page ${top("page")}
  device     ${top("device")}
  language   ${top("lang")}
`);
