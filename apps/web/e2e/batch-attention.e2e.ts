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

test("audio metadata is visible without offering an unsupported batch clean copy", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  // ID3v2.3 TPE1 (artist), followed by an MPEG Layer III frame header.
  const mp3 = Buffer.concat([
    Buffer.from([0x49, 0x44, 0x33, 3, 0, 0, 0, 0, 0, 16]),
    Buffer.from([0x54, 0x50, 0x45, 0x31, 0, 0, 0, 6, 0, 0, 0]),
    Buffer.from("Alice"),
    Buffer.from([0xff, 0xfb, 0x90, 0x64]),
  ]);
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "song.mp3", mimeType: "audio/mpeg", buffer: mp3 },
    { name: "unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
  ]);
  const batch = page.locator("#batch");
  await expect(batch.locator(".batch-row", { hasText: "song.mp3" })).toContainText("Who performed it");
  await expect(batch.getByRole("checkbox", { name: /Make a clean copy of song.mp3/ })).toHaveCount(0);
});
