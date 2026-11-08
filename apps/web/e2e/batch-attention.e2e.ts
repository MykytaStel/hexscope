import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";

test("a batch shows value-free format coverage and can focus incomplete reads", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  const photo = readFileSync(new URL("../../../crates/hexscope-core/tests/fixtures/photo.png", import.meta.url));
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "private-photo.png", mimeType: "image/png", buffer: photo },
    { name: "unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
  ]);

  const batch = page.locator("#batch");
  const coverage = batch.locator(".batch-coverage");
  await expect(coverage.locator("[data-coverage-format=png]")).toContainText("complete");
  await expect(coverage.locator("[data-coverage-format=unknown]")).toContainText("unsupported");
  await expect(coverage).not.toContainText("private-photo.png");
  await batch.getByRole("button", { name: /Incomplete/ }).click();
  await expect(batch.locator(".batch-row", { hasText: "unknown.bin" })).toBeVisible();
  await expect(batch.locator(".batch-row", { hasText: "private-photo.png" })).toHaveCount(0);
});

test("a batch downloads a versioned coverage report without input names", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  const photo = readFileSync(new URL("../../../crates/hexscope-core/tests/fixtures/photo.png", import.meta.url));
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "private-photo.png", mimeType: "image/png", buffer: photo },
    { name: "unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
  ]);

  const coverage = page.locator("#batch .batch-coverage");
  await coverage.locator("summary").click();
  await expect(coverage.getByRole("button", { name: "Save coverage report", exact: true })).toBeVisible();
  await expect(coverage).toContainText("no file names or findings");
  const downloadPromise = page.waitForEvent("download");
  await coverage.getByRole("button", { name: "Save coverage report", exact: true }).click();
  const download = await downloadPromise;

  expect(download.suggestedFilename()).toBe("hexscope-batch-coverage.json");
  const report = JSON.parse(readFileSync((await download.path())!, "utf8"));
  expect(report).toEqual({
    schema: "hexscope.batch-coverage",
    version: 1,
    files: { total: 2, read: 2, not_read: 0 },
    formats: [
      { format: "png", files: 1, complete: 1, partial: 0, unsupported: 0 },
      { format: "unknown", files: 1, complete: 0, partial: 0, unsupported: 1 },
    ],
  });
  expect(JSON.stringify(report)).not.toContain("private-photo");
});

test("a batch saves readable coverage and diagnostic CSV reports", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  const photo = readFileSync(new URL("../../../crates/hexscope-core/tests/fixtures/photo.png", import.meta.url));
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "private-photo.png", mimeType: "image/png", buffer: photo },
    { name: "private-unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
  ]);

  const coverage = page.locator("#batch .batch-coverage");
  await coverage.locator("summary").click();

  const coverageDownloadPromise = page.waitForEvent("download");
  await coverage.getByRole("button", { name: "Save coverage report as CSV" }).click();
  const coverageDownload = await coverageDownloadPromise;
  expect(coverageDownload.suggestedFilename()).toBe("hexscope-batch-coverage.csv");
  const coverageCsv = readFileSync((await coverageDownload.path())!, "utf8");
  expect(coverageCsv).toBe([
    "Format,Files,Complete,Partial,Unsupported,Total files,Read,Not read",
    "png,1,1,0,0,2,2,0",
    "unknown,1,0,0,1,2,2,0",
  ].join("\r\n"));
  expect(coverageCsv).not.toContain("private-photo");
  expect(coverageCsv).not.toContain("private-unknown");

  const diagnosticsDownloadPromise = page.waitForEvent("download");
  await coverage.getByRole("button", { name: "Save diagnostic report as CSV" }).click();
  const diagnosticsDownload = await diagnosticsDownloadPromise;
  expect(diagnosticsDownload.suggestedFilename()).toBe("hexscope-batch-diagnostics.csv");
  const diagnosticsCsv = readFileSync((await diagnosticsDownload.path())!, "utf8");
  expect(diagnosticsCsv).toBe([
    "Row,Format,Status,Total files",
    "2,unknown,unsupported,2",
  ].join("\r\n"));
  expect(diagnosticsCsv).not.toContain("private-photo");
  expect(diagnosticsCsv).not.toContain("private-unknown");
});

test("a phone can share the value-free batch coverage report directly", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "The share-sheet flow uses the mobile browser profile.");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: () => true });
    Object.defineProperty(navigator, "share", {
      configurable: true,
      value: async (data: { files?: File[] }) => {
        const file = data.files?.[0];
        if (!file) throw new Error("No file was shared");
        localStorage.setItem("hexscope.test.share-name", file.name);
        localStorage.setItem("hexscope.test.share-content", await file.text());
      },
    });
  });
  const photo = readFileSync(new URL("../../../crates/hexscope-core/tests/fixtures/photo.png", import.meta.url));
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "private-photo.png", mimeType: "image/png", buffer: photo },
    { name: "unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
  ]);

  const coverage = page.locator("#batch .batch-coverage");
  await coverage.locator("summary").click();
  await expect(coverage.getByRole("button", { name: "Share coverage report" })).toBeVisible();
  await coverage.getByRole("button", { name: "Share coverage report" }).click();

  expect(await page.evaluate(() => localStorage.getItem("hexscope.test.share-name"))).toBe("hexscope-batch-coverage.json");
  await expect.poll(() => page.evaluate(() => localStorage.getItem("hexscope.test.share-content"))).not.toBeNull();
  const content = await page.evaluate(() => localStorage.getItem("hexscope.test.share-content"));
  expect(JSON.parse(content!)).toMatchObject({ schema: "hexscope.batch-coverage", version: 1 });
  expect(content).not.toContain("private-photo");
  await expect(coverage.getByRole("button", { name: "Save coverage report", exact: true })).toBeVisible();
});

test("a phone can share a value-free diagnostic report for rows needing attention", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "The share-sheet flow uses the mobile browser profile.");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: () => true });
    Object.defineProperty(navigator, "share", {
      configurable: true,
      value: async (data: { files?: File[] }) => {
        const file = data.files?.[0];
        if (!file) throw new Error("No file was shared");
        localStorage.setItem("hexscope.test.share-name", file.name);
        localStorage.setItem("hexscope.test.share-content", await file.text());
      },
    });
  });
  const photo = readFileSync(new URL("../../../crates/hexscope-core/tests/fixtures/photo.png", import.meta.url));
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "private-photo.png", mimeType: "image/png", buffer: photo },
    { name: "private-unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from([1, 2, 3, 4]) },
  ]);

  const coverage = page.locator("#batch .batch-coverage");
  await coverage.locator("summary").click();
  await expect(coverage.getByRole("button", { name: "Share diagnostic report" })).toBeVisible();
  await coverage.getByRole("button", { name: "Share diagnostic report" }).click();

  expect(await page.evaluate(() => localStorage.getItem("hexscope.test.share-name"))).toBe("hexscope-batch-diagnostics.json");
  await expect.poll(() => page.evaluate(() => localStorage.getItem("hexscope.test.share-content"))).not.toBeNull();
  const content = await page.evaluate(() => localStorage.getItem("hexscope.test.share-content"));
  expect(JSON.parse(content!)).toEqual({
    schema: "hexscope.batch-diagnostics",
    version: 1,
    total: 2,
    issues: [{ index: 2, format: "unknown", status: "unsupported" }],
  });
  expect(content).not.toContain("private-unknown");
  await expect(coverage.getByRole("button", { name: "Save diagnostic report", exact: true })).toBeVisible();
});

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

test("failed batch reads can be retried without rereading completed files", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    const state = window as typeof window & { __failedParseOnce?: boolean; __parseAttempts?: number };
    state.__failedParseOnce = false;
    state.__parseAttempts = 0;
    const NativeWorker = window.Worker;
    class FlakyWorker extends NativeWorker {
      postMessage(message: any, transfer?: Transferable[]): void;
      postMessage(message: any, options?: StructuredSerializeOptions): void;
      postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (message?.type === "parse" && ["retry.png", "already-read.png"].includes(message.file?.name)) {
          state.__parseAttempts = (state.__parseAttempts ?? 0) + 1;
          if (message.file?.name === "retry.png" && !state.__failedParseOnce) {
            state.__failedParseOnce = true;
            setTimeout(() => this.dispatchEvent(new ErrorEvent("error")), 0);
            return;
          }
        }
        if (Array.isArray(transferOrOptions)) super.postMessage(message, transferOrOptions);
        else super.postMessage(message, transferOrOptions);
      }
    }
    window.Worker = FlakyWorker;
  });
  const photo = readFileSync(new URL("../../../crates/hexscope-core/tests/fixtures/photo.png", import.meta.url));
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles([
    { name: "retry.png", mimeType: "image/png", buffer: photo },
    { name: "already-read.png", mimeType: "image/png", buffer: photo },
  ]);

  const batch = page.locator("#batch");
  const retry = batch.getByRole("button", { name: "Retry failed files" });
  await expect(retry).toBeVisible();
  expect(await page.evaluate(() => (window as typeof window & { __parseAttempts: number }).__parseAttempts)).toBe(2);
  await retry.click();
  await expect(retry).toHaveCount(0);
  expect(await page.evaluate(() => (window as typeof window & { __parseAttempts: number }).__parseAttempts)).toBe(3);
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
