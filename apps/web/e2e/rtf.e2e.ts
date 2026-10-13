// A Rich Text document, as Word saves one: who made it and saved it last,
// the company, the dates, the program and template, its tracked changes and
// comments — and a clean copy that accepts the changes and is confirmed clean.
import { expect, test } from "@playwright/test";

test("an RTF document says who made and changed it, and its clean copy loses all of it", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  page.on("download", (download) => void download.cancel());
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles(new URL("./fixtures/lease.rtf", import.meta.url).pathname);
  await expect(page.locator(".verdict-title")).toHaveText("This document gives away 15 things");
  const facts = page.locator(".reveal-list");
  await expect(facts).toContainText("Олена Koval");
  await expect(facts).toContainText("C:\\Users\\olena\\Templates\\Lease.dotx");
  await expect(facts).toContainText("18 000 UAH; offer that first");

  await page.locator(".verdict-cta").click();
  const after = page.locator(".ba-side.is-after");
  await expect(after.locator(".ba-count")).toHaveText("15");
  await expect(after.locator(".ba-what")).toHaveText("removed — nothing is left in the copy");
  await expect(after).toHaveClass(/is-clear/);
});
