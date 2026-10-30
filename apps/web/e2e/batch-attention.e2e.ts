import { expect, test } from "@playwright/test";

test("a file that cannot be inspected stays visible in the attention filter", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
    { name: "empty.txt", mimeType: "text/plain", buffer: Buffer.from("hello") },
  ]);
  const batch = page.locator("#batch");
  await expect(batch.locator(".batch-summary")).toContainText("not fully checked");
  await expect(batch.locator(".batch-summary")).not.toContainText("No personal metadata found");
  await batch.getByRole("button", { name: /Needs attention/ }).click();
  await expect(batch.locator(".batch-row", { hasText: "unknown.bin" })).toBeVisible();
});
