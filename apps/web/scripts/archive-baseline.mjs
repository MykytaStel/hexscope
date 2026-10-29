// What a large archive costs the page: opens ZIP files on a computer's and a
// phone's profile and records the time to the answer, long tasks, the main
// thread's heap, DOM size, the structure tree, a search for the last entry,
// and the worker requests and byte reads each step causes. Run it before and
// after a change to archives, against the built site:
//
//   node scripts/archive-baseline.mjs http://localhost:4174/ a.zip b.zip [--out results.json]
//
// The phone profile is a Pixel 7 with the CPU slowed four times. The worker's
// own heap is not visible from the page and is not measured.
import { chromium, devices } from "@playwright/test";
import { basename } from "node:path";
import { writeFile } from "node:fs/promises";

const args = process.argv.slice(2);
const outAt = args.indexOf("--out");
const out = outAt >= 0 ? args.splice(outAt, 2)[1] : null;
const [base, ...archives] = args;
if (!base || archives.length === 0) {
  console.error("usage: node scripts/archive-baseline.mjs <base-url> <archive.zip>... [--out results.json]");
  process.exit(2);
}

const PROFILES = [
  { name: "computer", device: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } }, cpu: 1 },
  { name: "phone", device: devices["Pixel 7"], cpu: 4 },
];

/** Counts what the page asks the worker and its long tasks, from before the first script. */
function instrument() {
  localStorage.setItem("hexscope.language", "en");
  localStorage.setItem("hexscope.tour", "done");
  const m = (window.__baseline = { requests: {}, longTasks: 0, longTaskMs: 0, openStart: 0, ready: 0 });
  const send = Worker.prototype.postMessage;
  Worker.prototype.postMessage = function (message, ...rest) {
    if (message?.type) m.requests[message.type] = (m.requests[message.type] ?? 0) + 1;
    return send.call(this, message, ...rest);
  };
  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) { m.longTasks++; m.longTaskMs += e.duration; }
  }).observe({ type: "longtask" });
  new MutationObserver(() => {
    if (!m.ready && m.openStart && document.body?.dataset.state === "ready") m.ready = performance.now();
  }).observe(document, { attributes: true, subtree: true, attributeFilter: ["data-state"] });
}

async function measure(browser, profile, path) {
  const context = await browser.newContext(profile.device);
  const page = await context.newPage();
  await page.addInitScript(instrument);
  const workers = [];
  page.on("worker", (worker) => {
    workers.push(worker);
    // Entry bytes are read with Blob.slice in the worker; parsing reads the whole file without it.
    void worker.evaluate(() => {
      const slice = Blob.prototype.slice;
      self.__slices = 0;
      Blob.prototype.slice = function (...a) { self.__slices++; return slice.apply(this, a); };
    }).catch(() => {});
  });
  const slices = async () => (await Promise.all(workers.map((w) => w.evaluate(() => self.__slices ?? 0).catch(() => 0)))).reduce((a, b) => a + b, 0);
  const cdp = await context.newCDPSession(page);
  await page.goto(base);
  await page.locator("body[data-state=empty]").waitFor();
  if (profile.cpu > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate: profile.cpu });
  // The page's heap through the protocol; its peak is sampled between tasks, so it is a floor.
  const heap = async () => (await cdp.send("Runtime.getHeapUsage")).usedSize;
  await cdp.send("HeapProfiler.collectGarbage");
  const heapBefore = await heap();
  await page.evaluate(() => {
    const m = window.__baseline;
    m.requests = {}; m.longTasks = 0; m.longTaskMs = 0; m.openStart = performance.now();
  });

  // Opening: time from the file chosen to the answer on screen.
  let heapPeak = heapBefore, opening = true;
  const sampler = (async () => {
    while (opening) { heapPeak = Math.max(heapPeak, await heap().catch(() => 0)); await new Promise((ok) => setTimeout(ok, 50)); }
  })();
  await page.locator("#picker-empty").setInputFiles(path);
  await page.locator("body[data-state=ready]").waitFor({ timeout: 300_000 });
  await page.waitForTimeout(1500);
  opening = false; await sampler;
  await cdp.send("HeapProfiler.collectGarbage");
  const open = await page.evaluate(() => {
    const m = window.__baseline;
    return {
      ms: Math.round(m.ready - m.openStart),
      longTasks: m.longTasks,
      longTaskMs: Math.round(m.longTaskMs),
      elements: document.getElementsByTagName("*").length,
      requests: { ...m.requests },
    };
  });
  open.heapPeakGrowth = heapPeak - heapBefore;
  open.heapRetained = (await heap()) - heapBefore;
  open.domNodes = (await cdp.send("Memory.getDOMCounters")).nodes;
  open.workerSlices = await slices();

  // The structure view: the tree's rows on arrival and at the far end.
  const nav = page.locator("#app-nav");
  if (await nav.locator(".app-nav-more-actions [data-app-view='structure']").count()) await nav.locator(".app-nav-more > summary").click();
  await page.evaluate(() => { window.__baseline.longTasks = 0; window.__baseline.longTaskMs = 0; window.__baseline.t = performance.now(); });
  await nav.locator("[data-app-view='structure']").click();
  await page.locator(".tree .row").first().waitFor();
  const structure = await page.evaluate(async () => {
    const frames = () => new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
    await frames();
    const m = window.__baseline;
    const ms = Math.round(performance.now() - m.t);
    const rows = document.querySelectorAll(".tree .row").length;
    const host = document.querySelector(".tree").parentElement;
    const t = performance.now();
    host.scrollTop = host.scrollHeight;
    await frames();
    return { ms, rows, scrollEndMs: Math.round(performance.now() - t), rowsAtEnd: document.querySelectorAll(".tree .row").length, scrollHeight: host.scrollHeight, longTaskMs: Math.round(m.longTaskMs) };
  });

  // Finding the last entry by name: the page's search reads the bytes.
  const entries = Number(/archive-(\d+)k/.exec(basename(path))?.[1] ?? 0) * 1000;
  const last = `file-${String(entries - 1).padStart(6, "0")}.txt`;
  const before = await slices();
  await page.evaluate(() => { window.__baseline.requests = {}; window.__baseline.longTaskMs = 0; });
  await page.keyboard.press("/");
  const input = page.locator(".search input[type=search]");
  await input.waitFor({ state: "attached" });
  const search = await page.evaluate(async (query) => {
    const frames = () => new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
    const field = document.querySelector(".search input[type=search]");
    field.value = query;
    const t = performance.now();
    field.dispatchEvent(new Event("input"));
    const syncMs = performance.now() - t;
    await frames();
    const frameMs = performance.now() - t;
    await new Promise((ok) => setTimeout(ok, 800));
    return {
      query,
      syncMs: Math.round(syncMs),
      toFrameMs: Math.round(frameMs),
      count: document.querySelector(".search-count")?.textContent ?? "",
      selected: document.querySelector(".tree .row.is-selected .label")?.textContent ?? null,
      longTaskMs: Math.round(window.__baseline.longTaskMs),
      requests: { ...window.__baseline.requests },
    };
  }, last);
  search.workerSlices = (await slices()) - before;

  await context.close();
  return { profile: profile.name, archive: basename(path), entries, open, structure, search };
}

const browser = await chromium.launch();
const results = [];
for (const profile of PROFILES) {
  for (const path of archives) {
    const r = await measure(browser, profile, path);
    results.push(r);
    const mb = (n) => (n / 2 ** 20).toFixed(1);
    console.log(
      `${r.profile.padEnd(8)} ${r.archive.padEnd(18)} open ${r.open.ms} ms (long ${r.open.longTaskMs} ms, ${r.open.longTasks}) · heap peak +${mb(r.open.heapPeakGrowth)} MB, kept +${mb(r.open.heapRetained)} MB · ` +
        `${r.open.domNodes} DOM nodes · structure ${r.structure.ms} ms, ${r.structure.rows} rows (${r.structure.rowsAtEnd} at end, ${r.structure.scrollEndMs} ms) · ` +
        `search ${r.search.syncMs} ms sync, ${r.search.toFrameMs} ms to frame, "${r.search.count}" · requests ${JSON.stringify(r.open.requests)} / ${JSON.stringify(r.search.requests)} · slices ${r.open.workerSlices}/${r.search.workerSlices}`,
    );
  }
}
await browser.close();
if (out) await writeFile(out, JSON.stringify(results, null, 2) + "\n");
