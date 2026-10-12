// The messages people sent from the site — problems, ideas, thanks — read
// straight from the database with your Cloudflare login: only the account's
// owner can run it.
//
//   pnpm feedback            the ones not yet marked done, newest first
//   pnpm feedback all        every one
//   pnpm feedback done 12    mark message 12 done
import { execFileSync } from "node:child_process";
import { join } from "node:path";

const web = join(new URL("..", import.meta.url).pathname, "apps/web");
const d1 = (sql) => {
  const out = execFileSync("npx", ["--yes", "wrangler@4", "d1", "execute", "hexscope-counts", "--remote", "--json", "--command", sql], {
    cwd: web,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  return JSON.parse(out.slice(out.indexOf("[")))[0].results;
};

const [command, id] = process.argv.slice(2);
if (command === "done") {
  if (!/^\d+$/.test(id ?? "")) throw new Error("usage: pnpm feedback done <id>");
  d1(`UPDATE feedback SET done = 1 WHERE id = ${Number(id)}`);
  console.log(`Message ${id} marked done.`);
} else {
  let rows;
  try {
    rows = d1(`SELECT * FROM feedback ${command === "all" ? "" : "WHERE done = 0"} ORDER BY id DESC LIMIT 100`);
  } catch {
    rows = []; // No message yet: the table is made with the first one.
  }
  const marks = { bug: "Problem", idea: "Idea", thanks: "Thanks" };
  if (rows.length === 0) console.log(command === "all" ? "No messages yet." : "Nothing new.");
  for (const r of rows) {
    console.log(`\n#${r.id}  ${marks[r.kind] ?? r.kind}  ${r.day}  ${[r.lang, r.version, r.page, r.device].filter(Boolean).join(" · ")}${r.done ? "  (done)" : ""}`);
    console.log(r.message.replace(/^/gm, "  "));
    if (r.contact) console.log(`  — answer to: ${r.contact}`);
    if (r.report) console.log(`  — structure report, ${r.report.split("\n").length} lines:\n${r.report.replace(/^/gm, "    ")}`);
  }
}
