// Compares two builds element by element and, with --screenshots, pixel by
// pixel: every page, the home page with each sample open in each view, and
// the main tools and dialogs, on a computer and two narrow screens, in both
// themes. A stylesheet moved, merged or trimmed should leave both checks
// empty.
//
//   pnpm build && mv dist .local/dist-base     # the build to compare against
//   (change the stylesheets) && pnpm build
//   pnpm vite preview --outDir .local/dist-base --port 4177 --strictPort &
//   pnpm vite preview --port 4178 --strictPort &
//   node scripts/style-diff.mjs http://localhost:4177/ http://localhost:4178/ [--quick] [--only pages|app] [--grep name] [--screenshots] [--workers n]
//
// Includes representative hover and keyboard-focus states. Run it with both
// addresses the same first to check the screenshot comparison itself.
import { chromium, devices } from "@playwright/test";
import { readdirSync } from "node:fs";

const [baseA, baseB] = process.argv.slice(2, 4);
if (!baseA || !baseB) {
  console.error("usage: node scripts/style-diff.mjs <base url> <changed url> [--quick] [--only pages|app] [--grep name] [--screenshots] [--workers n]");
  process.exit(2);
}
const quick = process.argv.includes("--quick");
const screenshots = process.argv.includes("--screenshots");
const workersArg = process.argv.indexOf("--workers");
const workers = workersArg < 0 ? 4 : Number(process.argv[workersArg + 1]);
if (!Number.isInteger(workers) || workers < 1 || workers > 12) throw new Error("--workers must be an integer from 1 to 12");
const only = process.argv.includes("--only") ? process.argv[process.argv.indexOf("--only") + 1] : undefined;
// Only the states whose name has this in it: "--grep 'photo.jpg'".
const grep = process.argv.includes("--grep") ? process.argv[process.argv.indexOf("--grep") + 1] : undefined;
const dist = new URL("../dist/", import.meta.url);

const SCREENS = {
  computer: { viewport: { width: 1280, height: 800 } },
  phone: { ...devices["Pixel 7"] },
  // The narrowest phones, where the top bar draws its buttons closer.
  small: { ...devices["Pixel 7"], viewport: { width: 360, height: 760 } },
};
const THEMES = ["dark", "light"];
const VIEWS = ["overview", "metadata", "content", "structure", "bytes"];
const SAMPLES = [
  "photo.jpg", "redacted.pdf", "phishing.eml", "report.docx", "budget.xlsx", "report.pdf", "message.eml",
  "video.mov", "sample.png", "broken.png", "wifi.png", "hello.wasm", "plan.doc", "lease.odt", "photo.heic",
  "holiday.jpg", "film-negative.png", "ai-picture.png", "phishing.msg", "progressive.jpg", "cropped.jpg",
];

/** Every state to compare, as a name and what brings a fresh page to it. */
function scenarios() {
  const out = [];
  const sampleFile = (name) => new URL(`../public/samples/${name}`, import.meta.url).pathname;
  const openBatch = async (page) => {
    await page.locator("#picker-empty").setInputFiles(["photo.jpg", "cropped.jpg", "holiday.jpg"].map(sampleFile));
    await page.locator("body[data-state=batch]").waitFor();
  };
  if (only !== "app") {
    const html = (dir) => readdirSync(new URL(dir, dist)).filter((f) => f.endsWith(".html") && f !== "index.html");
    for (const f of html("./")) out.push({ name: f, path: f });
    for (const f of html("uk/")) out.push({ name: `uk/${f}`, path: `uk/${f}` });
    out.push({ name: "uk/index", path: "uk/" });
    // The keyboard's ring and a pointer over a button, on an article.
    out.push({ name: "support.html focus", path: "support.html", act: (p) => tab(p, 3) });
    out.push({ name: "support.html focus deep", path: "support.html", act: (p) => tab(p, 7) });
    out.push({ name: "support.html hover", path: "support.html", act: (p) => p.locator(".topbar .btn").first().hover() });
    out.push({ name: "support.html feedback", path: "support.html", act: (p) => p.locator("[data-feedback]").first().click() });
  }
  if (only !== "pages") {
    out.push({ name: "home", path: "" });
    out.push({ name: "home shortcuts", path: "", act: (p) => p.keyboard.press("?") });
    out.push({ name: "home feedback", path: "", act: (p) => p.locator("[data-feedback]").first().click() });
    out.push({ name: "home focus", path: "", act: (p) => tab(p, 4) });
    out.push({ name: "home hover", path: "", act: (p) => p.locator(".hero-actions .btn-primary").hover() });
    out.push({ name: "home batch", path: "", act: openBatch });
    out.push({ name: "home batch mosaic", path: "", act: async (p) => {
      await openBatch(p);
      await p.locator(".batch-tools > summary").click();
      await p.getByRole("button", { name: "Photo privacy mosaic" }).click();
      await p.locator(".photo-mosaic").waitFor();
    } });
    out.push({ name: "film roll", path: "", act: async (p) => {
      await p.locator("#film-roll-open").click();
      await p.locator(".film-roll[open]").waitFor();
    } });
    out.push({ name: "Film Lab", path: "?sample=film-negative.png", act: async (p) => {
      await p.locator(".more-details > summary").click();
      await p.locator(".film-lab-entry > summary").click();
      await p.locator(".film-lab[data-state=ready]").waitFor();
    } });
    out.push({
      name: "archive files",
      path: "",
      before: async (p) => {
        await p.locator("#picker-empty").setInputFiles(sampleFile("deflate-demo.zip"));
        await p.locator("body[data-state=ready]").waitFor();
      },
      view: "files",
      act: async (p) => p.locator(".pane-files [role=option]", { hasText: "rain.txt" }).click(),
    });
    out.push({ name: "photo.jpg focus", path: "?sample=photo.jpg", act: (p) => tab(p, 6) });
    for (const sample of quick ? SAMPLES.slice(0, 6) : SAMPLES) {
      for (const view of VIEWS) out.push({ name: `${sample} ${view}`, path: `?sample=${sample}`, view });
    }
    out.push({
      name: "photo.jpg clean copy",
      path: "?sample=photo.jpg",
      act: async (p) => {
        p.on("download", (d) => void d.cancel());
        await p.locator(".verdict-cta").click();
        await p.locator(".copy-verification").waitFor();
        await p.waitForTimeout(1500);
      },
    });
    out.push({
      name: "photo.jpg compare",
      path: "?sample=photo.jpg",
      act: async (p) => {
        const more = p.locator("#app-nav .app-nav-more");
        if (await more.count() && !(await more.evaluate((d) => d.open))) await more.locator("summary").click();
        await p.locator("#app-nav [data-app-action=compare]").click();
        await p.locator("#compare-picker").setInputFiles(sampleFile("cropped.jpg"));
        await p.locator(".compare[open]").waitFor();
      },
    });
  }
  return out;
}

/** Moves the keyboard's focus forward, as Tab does. */
async function tab(page, times) {
  for (let i = 0; i < times; i++) await page.keyboard.press("Tab");
}

/** The computed style of every element in the body, by its place in the page, as a hash. */
function capture() {
  const hash = (s) => {
    let h1 = 0xdeadbeef, h2 = 0x41c6ce57;
    for (let i = 0; i < s.length; i++) {
      const c = s.charCodeAt(i);
      h1 = Math.imul(h1 ^ c, 2654435761);
      h2 = Math.imul(h2 ^ c, 1597334677);
    }
    h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
    h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
    return (4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(36);
  };
  const styleOf = (el) => {
    const out = {};
    for (const pseudo of [null, "::before", "::after"]) {
      const cs = getComputedStyle(el, pseudo);
      if (pseudo && (cs.content === "none" || cs.content === "normal")) continue;
      for (let i = 0; i < cs.length; i++) out[(pseudo ?? "") + cs[i]] = cs.getPropertyValue(cs[i]);
    }
    return out;
  };
  const elements = new Map();
  const walk = (el, key) => {
    elements.set(key, el);
    let i = 0;
    for (const child of el.children) walk(child, `${key}>${child.tagName.toLowerCase()}${child.id ? `#${child.id}` : ""}:${i++}`);
  };
  walk(document.documentElement.querySelector("body"), "body");
  const hashes = {};
  // Tokens are implementation details; compare the resolved properties that
  // paint and position elements. --screenshots checks the rendered pixels too.
  // Canvas intrinsic ratios update asynchronously; the UI sizes its canvases
  // explicitly, and screenshot comparison catches any actual visual change.
  const text = (style, el) => Object.keys(style)
    .filter((p) => !p.replace(/^::(?:before|after)/, "").startsWith("--")
      && !(el.tagName === "CANVAS" && p === "aspect-ratio"))
    .sort()
    .map((p) => `${p}:${style[p]}`)
    .join(";");
  for (const [key, el] of elements) hashes[key] = hash(text(styleOf(el), el));
  window.__styled = { elements, styleOf };
  return hashes;
}

async function open(browser, base, screen, theme, scenario) {
  const context = await browser.newContext({ ...SCREENS[screen], baseURL: base, reducedMotion: "reduce", acceptDownloads: true });
  await context.addInitScript((t) => localStorage.setItem("hexscope.theme", t), theme);
  const page = await context.newPage();
  await page.goto(scenario.path, { waitUntil: "networkidle" });
  if (scenario.before) await scenario.before(page);
  if (scenario.path.startsWith("?sample=")) await page.locator("body[data-state=ready]").waitFor({ timeout: 30_000 });
  if (scenario.view) {
    const button = page.locator(`#app-nav [data-app-view='${scenario.view}']`);
    if (!(await button.count()) || !(await button.isEnabled())) {
      await context.close();
      return null;
    }
    if (await page.locator(`#app-nav .app-nav-more-actions [data-app-view='${scenario.view}']`).count()) {
      const more = page.locator("#app-nav .app-nav-more");
      if (!(await more.evaluate((d) => d.open))) await more.locator("summary").click();
    }
    if (!(await button.isVisible())) {
      await context.close();
      return null;
    }
    await button.click();
  }
  if (scenario.act) await scenario.act(page);
  // Mobile's "More" menu closes over the tree. Leaving the pointer on the
  // old menu item can hover a different tree row after the layout shifts.
  // That changes the inspector drawer between otherwise identical captures.
  if (!scenario.name.includes("hover")) await page.mouse.move(-10, -10);
  await page.waitForTimeout(700);
  await page.evaluate(async () => { await document.fonts.ready; });
  const fullPage = Boolean(scenario.path && !scenario.path.startsWith("?"));
  if (screenshots && fullPage) {
    await page.evaluate(async () => {
      const images = [...document.images];
      for (const image of images) image.loading = "eager";
      await Promise.all(images.map((image) => image.decode().catch(() => undefined)));
    });
  }
  // How long the parse took differs from run to run, and the width of the line that says it.
  await page.evaluate(() => {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) n.nodeValue = n.nodeValue.replace(/\d+(\.\d+)? ms\b/g, "1 ms");
  });
  return { context, page, fullPage, hashes: await page.evaluate(capture) };
}

async function pixelDifference(page, before, after) {
  return page.evaluate(async ({ before, after }) => {
    const bitmap = async (encoded) => {
      const binary = atob(encoded);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
      return createImageBitmap(new Blob([bytes], { type: "image/png" }));
    };
    const [a, b] = await Promise.all([bitmap(before), bitmap(after)]);
    if (a.width !== b.width || a.height !== b.height) {
      const result = { different: a.width * a.height, ratio: 1, size: `${a.width}×${a.height} → ${b.width}×${b.height}`, bounds: "image size" };
      a.close(); b.close();
      return result;
    }
    const width = a.width, height = a.height;
    const canvas = new OffscreenCanvas(width, height);
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) throw new Error("could not create screenshot comparison canvas");
    context.drawImage(a, 0, 0);
    const left = context.getImageData(0, 0, a.width, a.height).data;
    context.clearRect(0, 0, a.width, a.height);
    context.drawImage(b, 0, 0);
    const right = context.getImageData(0, 0, b.width, b.height).data;
    let different = 0, minX = a.width, minY = a.height, maxX = -1, maxY = -1;
    for (let i = 0; i < left.length; i += 4) {
      if (Math.max(Math.abs(left[i] - right[i]), Math.abs(left[i + 1] - right[i + 1]), Math.abs(left[i + 2] - right[i + 2]), Math.abs(left[i + 3] - right[i + 3])) <= 2) continue;
      different++;
      const pixel = i / 4, x = pixel % a.width, y = Math.floor(pixel / a.width);
      minX = Math.min(minX, x); minY = Math.min(minY, y); maxX = Math.max(maxX, x); maxY = Math.max(maxY, y);
    }
    a.close(); b.close();
    return {
      different,
      ratio: different / (width * height),
      size: `${width}×${height}`,
      bounds: different ? `${minX},${minY}–${maxX},${maxY}` : "none",
    };
  }, { before: before.toString("base64"), after: after.toString("base64") });
}

async function compare(browser, screen, theme, scenario) {
  const label = `${scenario.name} · ${screen} · ${theme}`;
  const [a, b] = await Promise.all([open(browser, baseA, screen, theme, scenario), open(browser, baseB, screen, theme, scenario)]);
  if (!a || !b) {
    await a?.context.close();
    await b?.context.close();
    return { label, skipped: true };
  }
  const differ = Object.keys(a.hashes).filter((k) => k in b.hashes && a.hashes[k] !== b.hashes[k]);
  const missing = Object.keys(a.hashes).filter((k) => !(k in b.hashes)).length + Object.keys(b.hashes).filter((k) => !(k in a.hashes)).length;
  const details = [];
  for (const key of differ.slice(0, 8)) {
    const read = (p) => p.evaluate((k) => {
      const el = window.__styled.elements.get(k);
      return { style: window.__styled.styleOf(el), tag: `${el.tagName.toLowerCase()}.${[...el.classList].join(".")}` };
    }, key);
    const [sa, sb] = await Promise.all([read(a.page), read(b.page)]);
    const props = Object.keys({ ...sa.style, ...sb.style }).filter((p) => sa.style[p] !== sb.style[p]);
    details.push({ key, tag: sa.tag, props: props.slice(0, 12).map((p) => `${p}: ${sa.style[p]} → ${sb.style[p]}`) });
  }
  let pixels;
  if (screenshots) {
    const options = { fullPage: a.fullPage, animations: "disabled", caret: "hide" };
    const [before, after] = await Promise.all([a.page.screenshot(options), b.page.screenshot(options)]);
    pixels = await pixelDifference(a.page, before, after);
  }
  await a.context.close();
  await b.context.close();
  return { label, elements: Object.keys(a.hashes).length, differ: differ.length, missing, details, pixels };
}

const browser = await chromium.launch();
const jobs = [];
for (const scenario of scenarios().filter((s) => !grep || s.name.includes(grep))) {
  for (const screen of quick ? ["computer"] : Object.keys(SCREENS)) {
    for (const theme of quick ? ["dark"] : THEMES) jobs.push({ scenario, screen, theme });
  }
}
let next = 0, changed = 0, done = 0, pixelStates = 0, pixelChanged = 0;
async function worker() {
  while (next < jobs.length) {
    const { scenario, screen, theme } = jobs[next++];
    let result;
    // A state that differs is read again: a canvas sized as it draws, or a tree still filling, settles the second time.
    for (let attempt = 0; attempt < 2 && (!result || result.differ || result.error || result.pixels?.different); attempt++) {
      try {
        result = await compare(browser, screen, theme, scenario);
      } catch (error) {
        result = { label: `${scenario.name} · ${screen} · ${theme}`, error: String(error).split("\n")[0] };
      }
    }
    done++;
    if (result.skipped) continue;
    if (result.error) {
      changed++;
      console.log(`✗ ${result.label}: ${result.error}`);
      continue;
    }
    let stateChanged = false;
    if (result.differ) {
      stateChanged = true;
      console.log(`✗ ${result.label}: ${result.differ} of ${result.elements} elements differ${result.missing ? `, ${result.missing} only in one` : ""}`);
      for (const d of result.details) console.log(`    ${d.tag}  (${d.key.split(">").slice(-3).join(">")})\n      ${d.props.join("\n      ")}`);
    } else if (result.missing) console.log(`· ${result.label}: same styles, ${result.missing} elements only in one`);
    if (result.pixels) {
      pixelStates++;
      if (result.pixels.different) {
        stateChanged = true;
        pixelChanged++;
        console.log(`✗ ${result.label}: screenshot differs at ${result.pixels.different} pixels (${(result.pixels.ratio * 100).toFixed(4)}% of ${result.pixels.size}; ${result.pixels.bounds})`);
      }
    }
    if (stateChanged) changed++;
  }
}
await Promise.all(Array.from({ length: workers }, worker));
await browser.close();
console.log(`${done} states compared, ${changed} differ${screenshots ? `; ${pixelStates} screenshots compared, ${pixelChanged} differ` : ""}`);
process.exit(changed ? 1 : 0);
