// A TIFF, as a scanner or camera saves it: what its tags give away, and a
// clean copy, made in place, confirmed to have lost all of it.
import { expect, test } from "@playwright/test";

test("a TIFF says where, with what and by whom, and its clean copy says nothing", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles(new URL("./fixtures/photo.tif", import.meta.url).pathname);
  await expect(page.locator(".verdict-title")).toHaveText("This picture gives away 7 things");
  await expect(page.locator(".place-map svg")).toBeVisible();

  await page.locator(".verdict-cta").click();
  const after = page.locator(".ba-side.is-after");
  await expect(after.locator(".ba-count")).toHaveText("7");
  await expect(after.locator(".ba-what")).toHaveText("No findings found in the copy");
  await expect(after).toHaveClass(/is-clear/);
  await expect(after.locator(".ba-verification-item")).toHaveCount(0);
});
