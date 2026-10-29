// What Office on Windows leaves beside the properties: the folder a workbook
// was last saved in, a template's path, the printer, SharePoint's properties
// and a picture of the first page — each shown, and each gone from the copy.
import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  page.on("download", (download) => void download.cancel());
  await page.goto("./");
});

test("a workbook says the folder it was saved in, and its clean copy is confirmed without it", async ({ page }) => {
  await page.locator("#picker-empty").setInputFiles(new URL("./fixtures/windows.xlsx", import.meta.url).pathname);
  const facts = page.locator(".reveal-list");
  await expect(facts.locator("dt", { hasText: "Saved in" }).first()).toBeVisible();
  await expect(facts).toContainText("C:\\Users\\olena.koval\\Documents\\Salaries 2026\\");
  await expect(facts).toContainText("Canon iR-ADV C5535 Accounting");
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-what")).toHaveText("confirmed removed from the copy");
  await page.locator(".clean-removed > summary").click();
  await expect(page.locator(".clean-list")).toContainText("the folder it was last saved in");
  await expect(page.locator(".clean-list")).toContainText("the printer's name");
});

test("a Word document names its template's folder, the printer and SharePoint's people, and the copy loses them", async ({ page }) => {
  await page.locator("#picker-empty").setInputFiles(new URL("./fixtures/windows.docx", import.meta.url).pathname);
  const facts = page.locator(".reveal-list");
  await expect(facts).toContainText("C:\\Users\\olena.koval\\AppData\\Roaming\\Microsoft\\Templates\\Lease.dotx");
  await expect(facts).toContainText("SharePoint properties: olena.koval@northwind.example, Petro Shevchuk");
  await page.locator(".verdict-cta").click();
  await page.locator(".clean-removed > summary").click();
  const removed = page.locator(".clean-list");
  for (const said of ["the folder of its template", "SharePoint properties", "editing-session numbers", "a picture of the first page"]) {
    await expect(removed).toContainText(said);
  }
});
