// What a person does on the site, in a real browser: open a sample, read
// the answer, save a clean copy, check an email, go offline. Run on a
// computer's screen and a phone's (playwright.config.ts).
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";

// Whatever the page throws, and whatever its Content-Security-Policy blocks,
// fails the test that met it.
let problems: string[] = [];
test.beforeEach(({ page }) => {
  problems = [];
  page.on("pageerror", (e) => problems.push(`error: ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error" && !/net::ERR_INTERNET_DISCONNECTED|Failed to load resource/.test(m.text())) problems.push(`console: ${m.text()}`);
  });
  page.on("load", () =>
    void page
      .evaluate(() => document.addEventListener("securitypolicyviolation", (e) => console.error(`CSP blocked ${e.violatedDirective}: ${e.blockedURI}`)))
      .catch(() => {}),
  );
});
test.afterEach(() => expect(problems).toEqual([]));

/** The landing page, once its script is running: its doors and pickers are in the page before it. */
async function home(page: Page): Promise<void> {
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
}

/** Opens a sample from its door on the landing page. */
async function openDoor(page: Page, door: RegExp): Promise<void> {
  await home(page);
  await page.getByRole("button", { name: door }).click();
  await expect(page.locator(".verdict-title")).toBeVisible();
}

/** Downloads are written nowhere: each is noted and cancelled. */
function catchDownloads(page: Page): string[] {
  const names: string[] = [];
  page.on("download", (d) => {
    names.push(d.suggestedFilename());
    void d.cancel();
  });
  return names;
}

test("the landing page holds still while its demonstration plays", async ({ page }) => {
  await page.goto("./");
  await expect(page.locator(".demo")).toBeVisible();
  const below = page.locator(".doors-title").first();
  const tops: number[] = [];
  for (let i = 0; i < 16; i++) {
    tops.push((await below.boundingBox())!.y);
    await page.waitForTimeout(500);
  }
  // A few pixels once, while the first scene settles; never the jumps of a card resizing.
  expect(Math.max(...tops) - Math.min(...tops)).toBeLessThanOrEqual(12);
});

test("nothing scrolls sideways", async ({ page }) => {
  await page.goto("./");
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
  await openDoor(page, /Check a photo/);
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
});

test("a photo: the answer, where it was taken, and a clean copy with nothing left", async ({ page }) => {
  const saved = catchDownloads(page);
  await openDoor(page, /Check a photo/);
  await expect(page.locator(".verdict-title")).toHaveText(/^This photo gives away \d+ things$/);
  // Where the keyboard starts: the answer.
  await expect(page.locator(".verdict-title")).toBeFocused();
  await expect(page.locator(".place-map svg")).toBeVisible();

  await page.locator(".verdict-cta").click();
  const after = page.locator(".ba-side.is-after .ba-count");
  await expect(after).toHaveText("0");
  await expect(page.locator(".ba-side.is-after")).toHaveClass(/is-clear/);
  // A computer saves it; a phone may offer to share it instead.
  if (saved.length > 0) expect(saved).toEqual(["photo-clean.jpg"]);
});

test("a fake bank email: why it may not be real, drawn on its way", async ({ page }) => {
  await openDoor(page, /Is this email real/);
  await expect(page.locator(".verdict-title")).toHaveText("This email may not be from who it says");
  await expect(page.locator(".mail-route .mail-stop").first()).toContainText("security@example-bank.com");
  await expect(page.locator(".mail-off .mail-stop.is-bad")).toHaveCount(3);
});

test("a blacked-out PDF: the names still under its boxes", async ({ page }) => {
  await openDoor(page, /Check a document before you send it/);
  await expect(page.locator(".verdict-title")).toHaveText(/^This PDF gives away \d+ things$/);
  await expect(page.locator(".reveal-list")).toContainText("Olena Koval");
});

test("a spreadsheet's clean copy keeps what is part of it, and says so", async ({ page }) => {
  catchDownloads(page);
  await page.goto("./?sample=budget.xlsx");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-what")).toContainText("part of the document itself");
});

test("a file opened earlier in the tab opens again from the list", async ({ page }) => {
  await openDoor(page, /Check a photo/);
  await page.locator("#picker").setInputFiles(new URL("../public/samples/phishing.eml", import.meta.url).pathname);
  await expect(page.locator(".verdict-title")).toHaveText("This email may not be from who it says");
  const again = page.locator(".recent-files button", { hasText: "photo.jpg" });
  await again.click();
  await expect(page.locator(".verdict-title")).toHaveText(/^This photo gives away/);
  // It lives in memory only: a new page load forgets it.
  await page.reload();
  await expect(page.locator(".recent-files")).toHaveCount(0);
});

test("a photo copied as a clean picture: pixels only, nothing else", async ({ page, context }, info) => {
  test.skip(info.project.name !== "computer", "the clipboard is granted on the computer's browser");
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openDoor(page, /Check a photo/);
  await page.getByRole("button", { name: "Copy a clean picture" }).first().click();
  await expect(page.getByRole("button", { name: /Copied/ })).toBeVisible();
  const chunks = await page.evaluate(async () => {
    const [item] = await navigator.clipboard.read();
    const b = new Uint8Array(await (await item.getType("image/png")).arrayBuffer());
    const seen: string[] = [];
    for (let i = 8; i + 8 <= b.length; ) {
      const len = ((b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]) >>> 0;
      seen.push(String.fromCharCode(...b.subarray(i + 4, i + 8)));
      i += 12 + len;
    }
    return [...new Set(seen)];
  });
  // No text, no EXIF, no XMP: the picture and nothing else.
  expect(chunks.filter((c) => !["IHDR", "IDAT", "IEND", "sRGB", "gAMA", "cHRM", "pHYs", "iCCP"].includes(c))).toEqual([]);
});

test("the keyboard list closes every way people try: ×, Escape, a click outside, ? again", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "keys are for a computer");
  await home(page);
  const list = page.locator("dialog.shortcuts");
  const open = async () => {
    await page.keyboard.press("?");
    await expect(list).toBeVisible();
  };
  await open();
  await page.getByRole("button", { name: "Close", exact: true }).first().click();
  await expect(list).toHaveCount(0);
  await open();
  await page.keyboard.press("Escape");
  await expect(list).toHaveCount(0);
  await open();
  await page.mouse.click(5, 5);
  await expect(list).toHaveCount(0);
  await open();
  await page.keyboard.press("?");
  await expect(list).toHaveCount(0);
});

const sample = (name: string) => new URL(`../public/samples/${name}`, import.meta.url).pathname;

test("several files: listed, then clean copies of those that give something away", async ({ page }, info) => {
  const saved = catchDownloads(page);
  await home(page);
  await page.locator("#picker-empty").setInputFiles([sample("photo.jpg"), sample("report.docx"), sample("sample.png")]);
  const list = page.locator("#batch");
  await expect(list).toContainText("photo.jpg");
  await expect(list).toContainText("report.docx");
  await expect(page.locator(".batch-title")).toHaveText("2 of 3 files need a look");
  await expect(page.locator(".batch-row", { hasText: "sample.png" }).locator(".batch-answer")).toHaveText("Nothing personal found");
  const save = page.locator("#batch .btn-clean");
  await expect(save).toBeEnabled();
  await save.click();
  if (info.project.name === "computer") {
    await expect.poll(() => saved).toEqual(["hexscope-clean-copies.zip"]);
    await expect(list).toContainText(/Saved \d+ clean cop/);
  } else {
    // A phone that cannot share files saves the archive too.
    await expect(list).toContainText(/clean cop/);
  }
});

test("a broken picture: what is wrong, and a repaired copy", async ({ page }) => {
  const saved = catchDownloads(page);
  await openDoor(page, /Why won't it open/);
  await expect(page.locator(".verdict-title")).toHaveText("This picture is damaged");
  await page.locator(".btn-repair").click();
  await expect.poll(() => saved).toEqual(["broken-repaired.png"]);
});

test("after one visit, it works offline — a document too", async ({ page, context }) => {
  await page.goto("./");
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready;
    // The install keeps the whole build: wait until the full parser is in.
    for (let i = 0; i < 100; i++) {
      const keys = await caches.keys();
      const cache = keys.find((k) => k.startsWith("hexscope-") && k !== "hexscope-shared");
      if (cache) {
        const urls = (await (await caches.open(cache)).keys()).map((r) => r.url);
        if (urls.filter((u) => u.endsWith(".wasm")).length >= 2 && urls.some((u) => u.endsWith("redacted.pdf"))) return;
      }
      await new Promise((r) => setTimeout(r, 200));
    }
    throw new Error("the app was not kept for offline use");
  });
  await context.setOffline(true);
  await page.reload();
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.getByRole("button", { name: /Check a document before you send it/ }).click();
  await expect(page.locator(".verdict-title")).toHaveText(/^This PDF gives away/);
  await page.goto("./black-out-a-pdf.html");
  await expect(page).toHaveTitle(/black out a PDF/i);
});

/** The sample video with its picture and sound grown to `size` bytes, written where the test keeps its files. */
function longVideo(path: string, size: number): string {
  const src = readFileSync(sample("video.mov"));
  const parts: Buffer[] = [];
  for (let at = 0; at + 8 <= src.length; ) {
    const len = src.readUInt32BE(at);
    if (src.toString("latin1", at + 4, at + 8) === "mdat") {
      const box = Buffer.alloc(8 + size);
      box.writeUInt32BE(8 + size, 0);
      src.copy(box, 4, at + 4, at + len);
      parts.push(box);
    } else parts.push(src.subarray(at, at + len));
    at += len;
  }
  writeFileSync(path, Buffer.concat(parts));
  return path;
}

test("a long video: read without its picture and sound, and cleaned all the same", async ({ page }, info) => {
  const path = longVideo(info.outputPath("long-video.mov"), 80 << 20);
  await home(page);
  await page.locator("#picker-empty").setInputFiles(path);
  await expect(page.locator(".verdict-title")).toHaveText(/^This video gives away \d+ things$/);
  await expect(page.locator("dt", { hasText: "Read" }).locator("+ dd")).toContainText("All but the picture and sound");

  const download = page.waitForEvent("download").catch(() => null);
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-count")).toHaveText("0");
  const d = info.project.name === "computer" ? await download : null;
  if (d) {
    expect(d.suggestedFilename()).toBe("long-video-clean.mov");
    expect(statSync(await d.path()).size).toBe(statSync(path).size);
  }
});

test("the bytes have a bar of their own: nothing sits on them", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "the bytes are a tab of their own on a phone");
  await openDoor(page, /Check a photo/);
  await page.locator("#viewswitch button", { hasText: "Bytes" }).click();
  const bytes = (await page.locator(".hex-scroller").boundingBox())!;
  for (const part of [".hex-legend", ".search-open"]) {
    const box = (await page.locator(part).boundingBox())!;
    expect(box.y + box.height, part).toBeLessThanOrEqual(bytes.y);
  }
  await page.keyboard.press("/");
  await page.keyboard.type("hexscope");
  await expect(page.locator(".search-count")).toHaveText(/^1 of \d+$/);
});

test("an Outlook message is read as an email, and an old Word file is cleaned", async ({ page }) => {
  const saved = catchDownloads(page);
  await page.goto("./?sample=phishing.msg");
  await expect(page.locator(".verdict-title")).toHaveText("This email may not be from who it says");
  await expect(page.locator(".reveal-list")).toContainText("invoice.pdf.html");

  await page.goto("./?sample=plan.doc");
  await expect(page.locator(".verdict-title")).toHaveText("This document gives away 2 things");
  await expect(page.locator(".reveal-list")).toContainText("Olena Koval");
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-count")).toHaveText("0");
  if (saved.length > 0) expect(saved).toEqual(["plan-clean.doc"]);
});

test("a QR code: a Wi-Fi password named, and blacked out in a new picture", async ({ page }) => {
  const saved = catchDownloads(page);
  await page.goto("./?sample=wifi.png");
  await expect(page.locator(".verdict-title")).toHaveText("This picture gives away 1 thing");
  await expect(page.locator(".reveal-list")).toContainText("its password “correct horse battery staple”");
  await page.getByRole("button", { name: "Black out the QR code…" }).click();
  await page.getByRole("button", { name: "Make the copy" }).click();
  await page.getByRole("button", { name: "Open the copy" }).click();
  await expect(page.locator(".verdict-title")).toHaveText("Nothing personal found in this picture");
  if (saved.length > 0) expect(saved).toEqual(["wifi-blacked-out.png"]);
});

test("a QR code in a PDF that goes to a lookalike address", async ({ page }) => {
  await home(page);
  await page.locator("#picker-empty").setInputFiles(new URL("../src/qr/fixtures/invoice.pdf", import.meta.url).pathname);
  await expect(page.locator(".reveal-list")).toContainText("shown as “exаmple-bank.com”");
  await expect(page.locator(".reveal-list")).toContainText("in the picture on page 1");
});

test("a QR code a PDF draws in boxes, and an Outlook message kept as RTF", async ({ page }) => {
  await home(page);
  await page.locator("#picker-empty").setInputFiles(new URL("../src/qr/fixtures/vector-qr.pdf", import.meta.url).pathname);
  await expect(page.locator(".reveal-list")).toContainText("in a code drawn on page 1");
  await page.locator("#picker").setInputFiles(new URL("../../../crates/hexscope-core/tests/fixtures/phishing-rtf.msg", import.meta.url).pathname);
  await expect(page.locator(".verdict-title")).toHaveText("This email may not be from who it says");
  await expect(page.locator(".reveal-list")).toContainText("goes to login.example.info");
});
