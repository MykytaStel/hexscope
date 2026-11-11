import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { zipOf } from "./zips";
import { openView } from "./views";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
});

test("Core plan and rules work before saving a photo", async ({ page }) => {
  await page.goto("./?sample=photo.jpg");
  const tools = page.locator(".preflight-disclosure");
  await tools.locator("summary").first().click();
  await expect(tools).toContainText("Plan changes before making a copy");
  await tools.getByRole("button", { name: "Check these rules" }).click();
  await expect(tools.getByRole("status").first()).toContainText("Rules need attention");
  await tools.getByRole("button", { name: "Try a damaged sample" }).click();
  await expect(tools).toContainText("Your file was not changed.");
  expect((await new AxeBuilder({ page }).include(".preflight-disclosure").analyze()).violations).toEqual([]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test("a verified copy downloads together with its private receipt and summary", async ({ page }) => {
  page.on("download", (download) => { if (download.suggestedFilename() !== "hexscope-publication.zip") void download.cancel(); });
  await page.goto("./?sample=photo.jpg");
  await page.locator(".verdict-cta").click();
  const button = page.getByRole("button", { name: "Save copy and report as ZIP" });
  await expect(button).toBeVisible();
  const pending = page.waitForEvent("download", (download) => download.suggestedFilename() === "hexscope-publication.zip");
  await button.click();
  const path = await (await pending).path();
  const bytes = await readFile(path!);
  expect(bytes.toString()).toContain("summary.html");
  expect(bytes.toString()).toContain("hexscope-receipt.json");
  expect(bytes.toString()).not.toContain("HX-000042");
});

test("PDF selected text redaction finishes with checked output", async ({ page }) => {
  page.on("download", (download) => void download.cancel());
  await page.goto("./?sample=report.pdf");
  const editor = page.locator(".redactor");
  await editor.locator('input[type="search"]').fill("Salary");
  await editor.getByRole("button", { name: "Find", exact: true }).click();
  await expect(editor.locator("mark.redact-hit")).toContainText("Salary");
  await editor.locator(".redact-form button").click();
  await editor.locator(".btn-primary").click();
  await expect(editor.locator(".copy-verification")).toContainText("Selected PDF text");
});

test("ZIP entries remain navigable on every engine", async ({ page }) => {
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles({ name: "bundle.zip", mimeType: "application/zip", buffer: zipOf([{ name: "docs/readme.txt", data: "sample" }]) });
  await page.locator("body[data-state=ready]").waitFor();
  await openView(page, "files");
  await expect(page.locator(".pane-files .archive-name")).toContainText(["docs"]);
});

test("cleaning checks the actual copy against the source publication profile", async ({ page }) => {
  page.on("download", (download) => void download.cancel());
  await page.goto("./?sample=photo.jpg");
  const tools = page.locator(".preflight-disclosure");
  await tools.locator("summary").first().click();
  await tools.getByRole("button", { name: "Check these rules" }).click();
  await expect(tools.getByRole("button", { name: /Show evidence.*Location/ })).toBeVisible();
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".copy-policy")).toHaveText("Copy passed the publication profile");
});
