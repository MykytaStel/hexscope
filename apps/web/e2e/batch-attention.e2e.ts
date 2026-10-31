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

test("MP3 metadata is visible and can be selected for a batch clean copy", async ({ page }) => {
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
  await expect(batch.getByRole("checkbox", { name: /Make a clean copy of song.mp3/ })).toBeVisible();
});

test("a FLAC with complete Vorbis comments can be selected for a batch clean copy", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  const vendor = Buffer.from("test encoder");
  const field = Buffer.from("TITLE=song");
  const comments = Buffer.concat([
    Buffer.from([vendor.length, 0, 0, 0]), vendor,
    Buffer.from([1, 0, 0, 0]),
    Buffer.from([field.length, 0, 0, 0]), field,
  ]);
  const flac = Buffer.concat([
    Buffer.from("fLaC"),
    Buffer.from([0, 0, 0, 34]), Buffer.alloc(34),
    Buffer.from([0x04, 0, 0, comments.length]), comments,
    Buffer.from([0x86, 0, 0, 4, 1, 2, 3, 4]),
    Buffer.from([0xff, 0xf8, 0x69, 0x08, 1, 2, 3, 4]),
  ]);
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "song.flac", mimeType: "audio/flac", buffer: flac },
    { name: "unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
  ]);
  const batch = page.locator("#batch");
  await expect(batch.locator(".batch-row", { hasText: "song.flac" })).toContainText("song");
  await expect(batch.getByRole("checkbox", { name: /Make a clean copy of song.flac/ })).toBeVisible();
});

test("a partial MP3 with no supported facts can still be selected for tag removal", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  const payload = Buffer.from([3, ...Buffer.from("custom\0value")]);
  const frame = Buffer.concat([
    Buffer.from("TXXX"),
    Buffer.from([0, 0, 0, payload.length, 0, 0]),
    payload,
  ]);
  const mp3 = Buffer.concat([
    Buffer.from([0x49, 0x44, 0x33, 4, 0, 0, 0, 0, 0, frame.length]),
    frame,
    Buffer.from([0xff, 0xfb, 0x90, 0x64, 1, 2, 3, 4]),
  ]);
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "partial.mp3", mimeType: "audio/mpeg", buffer: mp3 },
    { name: "unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
  ]);
  const batch = page.locator("#batch");
  const row = batch.locator(".batch-row", { hasText: "partial.mp3" });
  await expect(row).toContainText("Partial check");
  await expect(batch.getByRole("checkbox", { name: /Make a clean copy of partial.mp3/ })).toBeVisible();
});

test("APE text is shown while its audio metadata check remains partial", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  const item = Buffer.concat([Buffer.from([5, 0, 0, 0, 0, 0, 0, 0]), Buffer.from("Artist\0Alice")]);
  const footer = Buffer.alloc(32);
  footer.write("APETAGEX");
  footer.writeUInt32LE(2000, 8);
  footer.writeUInt32LE(item.length + 32, 12);
  footer.writeUInt32LE(1, 16);
  const mp3 = Buffer.concat([Buffer.from([0xff, 0xfb, 0x90, 0x64]), item, footer]);
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "ape.mp3", mimeType: "audio/mpeg", buffer: mp3 },
    { name: "unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
  ]);
  const batch = page.locator("#batch");
  const row = batch.locator(".batch-row", { hasText: "ape.mp3" });
  await expect(row).toContainText("Who performed it");
  await expect(row).toContainText("Partial check");
  await expect(batch.getByRole("checkbox", { name: /Make a clean copy of ape.mp3/ })).toBeVisible();
});
