import { expect, test, devices, type BrowserContext, type Page } from "@playwright/test";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, openSync, closeSync, ftruncateSync, writeSync, readFileSync, rmSync, writeFileSync, statSync } from "node:fs";
import { tmpdir, platform, arch, cpus, totalmem } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

interface ParseObservation {
  inputBytes: number;
  transferBytes: number;
  parseMs: number;
  rpcMs: number;
  compact: boolean;
}
interface Probe {
  parses: ParseObservation[];
  workers: number;
  terminated: number;
  longTasks: number[];
  readyAt: number;
}
type Metrics = Record<string, number | null>;
const root = fileURLToPath(new URL("../../../", import.meta.url));
const repeats = Number(process.env.HEXSCOPE_PERF_REPEATS ?? 3);
const profile = process.env.HEXSCOPE_PERF_PROFILE ?? "desktop";

async function setup(context: BrowserContext): Promise<void> {
  await context.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    const probe: Probe = { parses: [], workers: 0, terminated: 0, longTasks: [], readyAt: 0 };
    (window as unknown as { __hexscopePerf: Probe }).__hexscopePerf = probe;
    const NativeWorker = window.Worker;
    window.Worker = class extends NativeWorker {
      private requests = new Map<number, { start: number; size: number }>();
      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options);
        probe.workers++;
        super.addEventListener("message", (event: MessageEvent) => {
          const response = event.data;
          const request = this.requests.get(response?.id);
          if (!request || response.type === "progress") return;
          this.requests.delete(response.id);
          if (response.type !== "parsed") return;
          probe.parses.push({
            inputBytes: request.size,
            transferBytes: response.bytes.byteLength,
            parseMs: response.result.parseMs,
            rpcMs: performance.now() - request.start,
            compact: response.result.compactSize !== undefined,
          });
        });
      }
      override postMessage(message: any, transfer: Transferable[]): void;
      override postMessage(message: any, options?: StructuredSerializeOptions): void;
      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (message?.type === "parse" && message.file instanceof Blob) {
          this.requests.set(message.id, { start: performance.now(), size: message.file.size });
        }
        if (Array.isArray(transferOrOptions)) super.postMessage(message, transferOrOptions);
        else super.postMessage(message, transferOrOptions);
      }
      override terminate(): void {
        probe.terminated++;
        super.terminate();
      }
    };
    if (PerformanceObserver.supportedEntryTypes.includes("longtask")) {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) probe.longTasks.push(entry.duration);
      }).observe({ type: "longtask", buffered: true });
    }
    new MutationObserver((records) => {
      if (records.some(record => record.target === document.body) && document.body?.dataset.state === "ready") probe.readyAt = performance.now();
    }).observe(document, { subtree: true, attributes: true, attributeFilter: ["data-state"] });
  });
}

const snapshot = (page: Page) => page.evaluate(() => (window as unknown as { __hexscopePerf: Probe }).__hexscopePerf);
async function heap(context: BrowserContext, page: Page): Promise<number | null> {
  const cdp = await context.newCDPSession(page);
  try {
    const usage = await cdp.send("Runtime.getHeapUsage");
    return usage.usedSize;
  } finally {
    await cdp.detach();
  }
}

async function open(page: Page, context: BrowserContext, path: string, first: boolean): Promise<Metrics> {
  const previous = await snapshot(page);
  const start = await page.evaluate(() => performance.now());
  await page.locator(first ? "#picker-empty" : "#picker").setInputFiles(path);
  await page.waitForFunction((count) => {
    const p = (window as unknown as { __hexscopePerf: Probe }).__hexscopePerf;
    return p.parses.length > count && document.body.dataset.state === "ready" && p.readyAt > 0;
  }, previous.parses.length);
  const after = await snapshot(page);
  const parsed = after.parses.at(-1)!;
  expect(parsed.inputBytes).toBe(statSync(path).size);
  expect(parsed.parseMs).toBeGreaterThanOrEqual(0);
  expect(after.readyAt).toBeGreaterThanOrEqual(start);
  if (parsed.inputBytes >= 64 * 1024 * 1024) {
    expect(parsed.compact).toBe(true);
    expect(parsed.transferBytes).toBeLessThan(1024 * 1024);
  }
  const tasks = after.longTasks.slice(previous.longTasks.length);
  return {
    readyMs: Math.max(0, after.readyAt - start),
    rpcMs: parsed.rpcMs,
    parseMs: parsed.parseMs,
    transferBytes: parsed.transferBytes,
    pageHeapBytes: await heap(context, page),
    longTaskCount: tasks.length,
    maxLongTaskMs: Math.max(0, ...tasks),
  };
}

test("collect reproducible browser evidence", async ({ browser }, info) => {
  expect(Number.isInteger(repeats) && repeats >= 1 && repeats <= 20).toBe(true);
  expect(["desktop", "phone"]).toContain(profile);
  const directory = mkdtempSync(join(tmpdir(), "hexscope-perf-"));
  const large = join(directory, "large.mov");
  const largeWarm = join(directory, "large-warm.mov");
  const samples = [
    ["jpeg", "photo.jpg"], ["pdf", "report.pdf"], ["office", "report.docx"],
    ["zip", "deflate-demo.zip"], ["audio", "sample.mp3"],
  ].map(([id, name]) => ({ id, path: join(root, "apps/web/public/samples", name), warm: join(directory, "warm-" + name) }));
  const cases = [...samples, { id: "sparse-movie-64m", path: large, warm: largeWarm }];
  const observations: { id: string; inputBytes: number; inputSha256: string; metrics: Metrics[] }[] = [];
  try {
    const size = 64 * 1024 * 1024 + 20;
    const head = Buffer.alloc(20);
    head.writeUInt32BE(12, 0);
    head.write("ftyp", 4, "ascii");
    head.write("isom", 8, "ascii");
    head.writeUInt32BE(size - 12, 12);
    head.write("mdat", 16, "ascii");
    for (const path of [large, largeWarm]) {
      const fd = openSync(path, "w");
      try {
        ftruncateSync(fd, size);
        writeSync(fd, head, 0, head.length, 0);
      } finally { closeSync(fd); }
    }
    for (const item of samples) writeFileSync(item.warm, readFileSync(item.path));
    for (const item of cases) {
      const inputBytes = statSync(item.path).size;
      const inputSha256 = createHash("sha256").update(readFileSync(item.path)).digest("hex");
      const cold: Metrics[] = [], warm: Metrics[] = [];
      for (let i = 0; i < repeats; i++) {
        const context = await browser.newContext({
          ...(profile === "phone" ? devices["Pixel 7"] : { viewport: { width: 1280, height: 800 } }),
          serviceWorkers: "block",
        });
        try {
          await setup(context);
          const page = await context.newPage();
          await page.goto(info.config.metadata.baseURL as string);
          await expect(page.locator("body")).toHaveAttribute("data-state", "empty");
          cold.push(await open(page, context, item.path, true));
          warm.push(await open(page, context, item.warm, false));
        } finally { await context.close(); }
      }
      observations.push({ id: item.id + "/cold", inputBytes, inputSha256, metrics: cold }, { id: item.id + "/warm", inputBytes, inputSha256, metrics: warm });
    }

    // Batch mode: policy/QR can still use the realm until the queue releases it.
    const batchMetrics: Metrics[] = [];
    const photo = readFileSync(samples[0].path);
    const count = 10;
    const files = Array.from({ length: count }, (_, i) => ({ name: `bench-${i}.jpg`, mimeType: "image/jpeg", buffer: photo }));
    for (let i = 0; i < repeats; i++) {
      const context = await browser.newContext({ ...(profile === "phone" ? devices["Pixel 7"] : { viewport: { width: 1280, height: 800 } }), serviceWorkers: "block" });
      try {
        await setup(context);
        const page = await context.newPage();
        await page.goto(info.config.metadata.baseURL as string);
        await expect(page.locator("body")).toHaveAttribute("data-state", "empty");
        const start = await page.evaluate(() => performance.now());
        await page.locator("#picker-empty").setInputFiles(files);
        await expect(page.locator("#batch .batch-row")).toHaveCount(count);
        await expect(page.locator("#batch .batch-row:disabled")).toHaveCount(0, { timeout: 60_000 });
        await page.waitForFunction((n) => (window as unknown as { __hexscopePerf: Probe }).__hexscopePerf.parses.length === n, count);
        const end = await page.evaluate(() => performance.now());
        const probe = await snapshot(page);
        batchMetrics.push({ readyMs: end - start, workerCount: probe.workers, terminatedWorkers: probe.terminated, transferBytes: probe.parses.reduce((sum, p) => sum + p.transferBytes, 0), pageHeapBytes: await heap(context, page) });
      } finally { await context.close(); }
    }
    observations.push({ id: "batch-jpeg-10", inputBytes: count * photo.length, inputSha256: createHash("sha256").update(photo).digest("hex"), metrics: batchMetrics });
    const report = {
      schema: "hexscope.performance", version: 1, kind: "browser",
      revision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim(),
      environment: { platform: platform(), arch: arch(), cpu: cpus()[0]?.model, cores: cpus().length, ramBytes: totalmem(), node: process.version, browser: browser.version(), profile, repeats },
      cases: observations,
    };
    const output = resolve(root, process.env.HEXSCOPE_PERF_OUTPUT ?? `.perf/browser-${profile}.json`);
    mkdirSync(resolve(output, ".."), { recursive: true });
    writeFileSync(output, JSON.stringify(report, null, 2) + "\n");
    await info.attach("performance-evidence", { path: output, contentType: "application/json" });
    console.log(`Performance evidence: ${output}`);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
