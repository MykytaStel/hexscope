// What a test on an iPhone found. A photo as an iPhone saves it has, after
// the picture, a second one — the HDR gain map — that the file's MPF index
// lists: part of the photo, not something hidden, and its clean copy is
// confirmed clean all the same. And on a phone's screen: the clean copy's
// actions the same size, and the location badge showing the place.
import { expect, test } from "@playwright/test";

test("an iPhone photo's HDR gain map is not called hidden, and its clean copy is fully checked", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles(new URL("./fixtures/iphone-hdr.jpg", import.meta.url).pathname);
  await expect(page.locator(".verdict-title")).toHaveText("This photo gives away 8 things");
  await expect(page.locator(".verdict")).not.toContainText("Hidden in it");

  await page.locator(".verdict-cta").click();
  const after = page.locator(".ba-side.is-after");
  await expect(after.locator(".ba-count")).toHaveText("8");
  await expect(after.locator(".ba-what")).toHaveText("removed — nothing is left in the copy");
  await expect(after).toHaveClass(/is-clear/);
  await expect(page.locator(".clean-removed")).toContainText("A second picture for HDR screens or depth");
});

test("on a phone, the clean copy's actions stand one under another, the same size, sharing first", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the stacked actions are for narrow screens");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    Object.defineProperty(navigator, "canShare", { value: () => true });
  });
  await page.goto("./?sample=photo.jpg");
  await page.locator(".verdict-cta").click();
  const buttons = page.locator(".clean-actions .btn");
  await expect(buttons.first()).toHaveText("Share the clean copy");
  const boxes = await buttons.evaluateAll((all) => all.map((b) => [Math.round(b.getBoundingClientRect().width), Math.round(b.getBoundingClientRect().height)]));
  expect(boxes.length).toBeGreaterThanOrEqual(3);
  expect(new Set(boxes.map(([w]) => w)).size).toBe(1);
  expect(new Set(boxes.map(([, h]) => h)).size).toBe(1);
});

test("on a phone, the location badge shows the place in the summary, not the bytes", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "a computer's badge opens the bytes");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await page.locator("#location").click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "overview");
  await expect(page.locator(".reveal-list dt.is-flash")).toHaveText("Coordinates");
});

test("on a phone, several photos are cleaned with one button and shared with the next, in Ukrainian", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "sharing several copies at once is the phone's way");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    Object.defineProperty(navigator, "canShare", { value: () => true });
  });
  await page.goto("./");
  const photo = new URL("../public/samples/photo.jpg", import.meta.url).pathname;
  const iphone = new URL("./fixtures/iphone-hdr.jpg", import.meta.url).pathname;
  await page.locator("#picker-empty").setInputFiles([photo, iphone]);
  await expect(page.locator(".batch-title")).toHaveText("2 з 2 файлів варто переглянути");
  const make = page.locator(".batch-head .btn-clean");
  await expect(make).toHaveText("Зробити 2 чисті копії");
  await expect(page.locator(".batch-keep")).toBeHidden();
  await make.click();
  const share = page.locator(".batch-actions .btn-primary");
  await expect(share).toHaveText("Поділитися 2 чистими копіями");
  // Sharing is now the one thing to do.
  await expect(page.locator(".batch-head .btn-clean")).toHaveCount(0);
  await expect(page.locator(".batch-head .btn-again")).toBeVisible();
  await expect(page.locator("#batch")).toContainText("прибрано 8 · залишилося 0 · не перевірено 0");
});
