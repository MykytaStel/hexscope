// A file whose name says another format than its bytes: a Word document
// named .pdf, a LibreOffice spreadsheet named .xlsx. The verdict says what
// it is and the name that fits — the kind of document, not just "a ZIP".
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

const core = (name: string) => readFile(new URL(`../../../crates/hexscope-core/tests/fixtures/${name}`, import.meta.url));

test("a Word document named .pdf is said to be one, with the name that fits", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles({ name: "contract.pdf", mimeType: "application/pdf", buffer: await core("report.docx") });
  await expect(page.locator(".verdict")).toContainText(
    "Named .pdf, but it is a Word document. Apps that go by the name may refuse it or call it damaged; renaming it to .docx fixes that.",
  );
});

test("a LibreOffice spreadsheet named .xlsx is misnamed too, said in Ukrainian", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles({ name: "budget.xlsx", mimeType: "application/octet-stream", buffer: await core("budget.ods") });
  await expect(page.locator(".verdict")).toContainText("Названо .xlsx, але це таблиця LibreOffice.");
  await expect(page.locator(".verdict")).toContainText("перейменування на .ods це виправить");
});
