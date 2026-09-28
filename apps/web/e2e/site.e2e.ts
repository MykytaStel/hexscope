// What a person does on the site, in a real browser: open a sample, read
// the answer, save a clean copy, check an email, go offline. Run on a
// computer's screen and a phone's (playwright.config.ts).
import { expect, test, type Page } from "@playwright/test";

/** Opens a sample from its door on the landing page. */
async function openDoor(page: Page, door: RegExp): Promise<void> {
  await page.goto("./");
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
  await page.getByRole("button", { name: /Check a document before you send it/ }).click();
  await expect(page.locator(".verdict-title")).toHaveText(/^This PDF gives away/);
  await page.goto("./black-out-a-pdf.html");
  await expect(page).toHaveTitle(/black out a PDF/i);
});
