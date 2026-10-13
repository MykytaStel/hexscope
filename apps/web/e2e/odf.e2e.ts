// A LibreOffice document: who made it and when, the program and the system,
// who printed it and on which printer, the picture of its first page, its
// tracked changes and comments — and a clean copy confirmed to lose them.
import { expect, test } from "@playwright/test";

test("a LibreOffice document says who made, edited and printed it, and its clean copy loses all of it", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  page.on("download", (download) => void download.cancel());
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles(new URL("./fixtures/lease.odt", import.meta.url).pathname);
  await expect(page.locator(".verdict-title")).toHaveText("This document gives away 19 things");
  const facts = page.locator(".reveal-list");
  for (const label of ["Author", "Last saved by", "Printer", "Printed", "Thumbnail", "Deleted text"]) {
    await expect(facts.locator("dt", { hasText: label }).first()).toBeVisible();
  }
  await expect(facts).toContainText("HP LaserJet Pro M404 — Office 2nd floor");
  await expect(facts).toContainText("LibreOffice 7.6.4.1 on Linux (x86-64)");

  await page.locator(".verdict-cta").click();
  const after = page.locator(".ba-side.is-after");
  await expect(after.locator(".ba-count")).toHaveText("19");
  await expect(after.locator(".ba-what")).toHaveText("Confirmed removed");
  await page.locator(".clean-removed > summary").click();
  const removed = page.locator(".clean-list");
  await expect(removed.locator(".clean-part")).toContainText(["Thumbnails/thumbnail.png", "content.xml", "meta.xml", "settings.xml"]);
  await expect(removed).toContainText("2 tracked changes, accepted; 1 comment and who wrote them");
  // Its comments went with the copy: nothing is said to stay.
  await expect(page.locator(".cleaner")).not.toContainText("Kept:");
});
