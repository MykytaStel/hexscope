// A photo as an iPhone saves it: after the picture, a second one — the HDR
// gain map — that the file's MPF index lists. It is part of the photo, not
// something hidden, and the clean copy is confirmed clean all the same.
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
