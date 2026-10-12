// The visit counter's promise: nothing is counted from anywhere but the real site.
import { expect, test } from "@playwright/test";

test("sends no visit from anywhere but the real site", async ({ page }) => {
  const seen: string[] = [];
  page.on("request", (r) => {
    if (r.url().includes("/api/seen")) seen.push(r.url());
  });
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.goto("./your-files-stay-private");
  await page.waitForTimeout(2500);
  expect(seen).toEqual([]);
});
