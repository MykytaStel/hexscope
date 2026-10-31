// What a test on an iPhone found. A photo as an iPhone saves it has, after
// the picture, a second one — the HDR gain map — that the file's MPF index
// lists: part of the photo, not something hidden, and its clean copy is
// confirmed clean all the same. And on a phone's screen: the clean copy's
// actions the same size, and the location badge showing the place.
import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { StoredZip } from "../src/zipwrite";

test("on a phone, QR scanning still finds a code in a large photo", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the large-photo memory path is for a phone");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./");
  const qr = readFileSync(new URL("../src/qr/fixtures/H-78.png", import.meta.url));
  const pixels = await page.evaluate(async (sourceBytes) => {
    const source = await createImageBitmap(new Blob([new Uint8Array(sourceBytes)], { type: "image/png" }));
    const canvas = document.createElement("canvas");
    canvas.width = 3200;
    canvas.height = 2400;
    const context = canvas.getContext("2d")!;
    context.imageSmoothingEnabled = false;
    context.fillStyle = "white";
    context.fillRect(0, 0, canvas.width, canvas.height);
    const scale = Math.min(canvas.width / source.width, canvas.height / source.height);
    const width = source.width * scale;
    const height = source.height * scale;
    context.drawImage(source, (canvas.width - width) / 2, (canvas.height - height) / 2, width, height);
    source.close();
    const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob((value) => value ? resolve(value) : reject(new Error("PNG encoding failed")), "image/png"));
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  }, Array.from(qr));
  await page.locator("#picker-empty").setInputFiles({ name: "large-qr.png", mimeType: "image/png", buffer: Buffer.from(pixels) });
  await expect(page.locator(".verdict-title")).toHaveText("This picture gives away 1 thing");
  await expect(page.locator(".reveal-list")).toContainText("hexscope.pages.dev");
});

test("on a phone, QR scanning finds a code in a large picture inside a ZIP", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the large embedded-photo memory path is for a phone");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./");
  const qr = readFileSync(new URL("../src/qr/fixtures/H-78.png", import.meta.url));
  const pixels = await page.evaluate(async (sourceBytes) => {
    const source = await createImageBitmap(new Blob([new Uint8Array(sourceBytes)], { type: "image/png" }));
    const canvas = document.createElement("canvas");
    canvas.width = 3200;
    canvas.height = 2400;
    const context = canvas.getContext("2d")!;
    context.imageSmoothingEnabled = false;
    context.fillStyle = "white";
    context.fillRect(0, 0, canvas.width, canvas.height);
    const scale = Math.min(canvas.width / source.width, canvas.height / source.height);
    const width = source.width * scale;
    const height = source.height * scale;
    context.drawImage(source, (canvas.width - width) / 2, (canvas.height - height) / 2, width, height);
    source.close();
    const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob((value) => value ? resolve(value) : reject(new Error("PNG encoding failed")), "image/png"));
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  }, Array.from(qr));
  const zip = new StoredZip();
  await zip.add("large-qr.png", new Blob([Buffer.from(pixels)], { type: "image/png" }));
  const archive = zip.finish();
  await page.locator("#picker-empty").setInputFiles({ name: "large-qr.zip", mimeType: "application/zip", buffer: Buffer.from(await archive.arrayBuffer()) });
  await expect(page.locator(".reveal-list")).toContainText("hexscope.pages.dev");
});

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
  await expect(page.locator(".cleaner")).toContainText("Removes personal metadata from the main picture and its HDR gain map.");
  const after = page.locator(".ba-side.is-after");
  await expect(after.locator(".ba-count")).toHaveText("8");
  await expect(after.locator(".ba-what")).toHaveText("confirmed removed from the copy");
  await expect(after).toHaveClass(/is-clear/);
  await expect(page.locator(".clean-removed")).toContainText("A second picture for HDR screens or depth");
});

test("on a phone, the clean copy's actions stand one under another, the same size, sharing first", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the stacked actions are for narrow screens");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    Object.defineProperty(navigator, "canShare", { value: () => true });
    Object.defineProperty(navigator, "share", { value: async () => undefined });
  });
  await page.goto("./?sample=photo.jpg");
  await page.locator(".verdict-cta").click();
  const buttons = page.locator(".clean-actions .btn");
  await expect(buttons.first()).toHaveText("Share the clean copy");
  const boxes = await buttons.evaluateAll((all) => all.map((b) => [Math.round(b.getBoundingClientRect().width), Math.round(b.getBoundingClientRect().height)]));
  expect(boxes.length).toBeGreaterThanOrEqual(3);
  expect(new Set(boxes.map(([w]) => w)).size).toBe(1);
  expect(new Set(boxes.map(([, h]) => h)).size).toBe(1);
});

test("on a phone, file sharing works when the browser has share but no canShare", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "sharing a file is a phone action");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: undefined });
    Object.defineProperty(navigator, "share", {
      configurable: true,
      value: async ({ files }: ShareData) => {
        const file = files?.[0];
        if (!file) throw new Error("The clean photo was not passed to the share target");
        const state = window as typeof window & {
          __sharedFiles: { name: string; type: string; size: number; bytes: number[] }[];
        };
        state.__sharedFiles ??= [];
        state.__sharedFiles.push({
          name: file.name,
          type: file.type,
          size: file.size,
          bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
        });
      },
    });
  });
  const downloads: string[] = [];
  page.on("download", (download) => downloads.push(download.suggestedFilename()));
  await page.goto("./?sample=photo.jpg");
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".clean-actions")).toBeVisible();
  expect(downloads).toEqual([]);
  const share = page.locator(".clean-actions .btn").filter({ hasText: "Share the clean copy" });
  await expect(share).toBeVisible();
  await share.click();
  await expect.poll(() => page.evaluate(() => {
    const state = window as typeof window & { __sharedFiles?: unknown[] };
    return state.__sharedFiles?.length ?? 0;
  })).toBe(1);
  const sent = await page.evaluate(() => {
    const state = window as typeof window & { __sharedFiles: { name: string; type: string; size: number; bytes: number[] }[] };
    return state.__sharedFiles[0];
  });
  expect(sent.name).toBe("photo-clean.jpg");
  expect(sent.type).toBe("image/jpeg");
  expect(sent.size).toBeGreaterThan(0);
  expect(sent.bytes).toHaveLength(sent.size);
});

test("on a phone without file sharing, the saved photo explains how to share it from Files", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the fallback is for narrow screens");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: () => false });
    Object.defineProperty(navigator, "share", { configurable: true, value: undefined });
  });
  const downloads: string[] = [];
  page.on("download", (download) => downloads.push(download.suggestedFilename()));
  await page.goto("./?sample=photo.jpg");
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".clean-actions")).toBeVisible();
  await expect(page.locator(".clean-share-fallback")).toContainText("Копію вже збережено в «Завантаженнях»");
  await expect(page.locator(".clean-actions .share-fallback")).toHaveCount(0);
  expect(downloads).toEqual(["photo-clean.jpg"]);
});

test("when a phone share sheet rejects a file, its action changes to a save fallback", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the fallback is for narrow screens");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: undefined });
    Object.defineProperty(navigator, "share", {
      configurable: true,
      value: async () => { throw new DOMException("File sharing is unavailable", "NotAllowedError"); },
    });
  });
  await page.goto("./?sample=photo.jpg");
  await page.locator(".verdict-cta").click();
  await page.locator(".clean-actions .btn").filter({ hasText: "Share the clean copy" }).click();
  const save = page.locator(".clean-actions .share-fallback");
  await expect(save).toHaveText("Save a copy to share from Files");
  const downloadPromise = page.waitForEvent("download");
  await save.click();
  expect((await downloadPromise).suggestedFilename()).toBe("photo-clean.jpg");
});

test("when a phone share sheet is cancelled, the clean photo can still be shared or saved", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the share sheet is a phone action");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: () => true });
    Object.defineProperty(navigator, "share", {
      configurable: true,
      value: async () => { throw new DOMException("Share cancelled", "AbortError"); },
    });
  });
  await page.goto("./?sample=photo.jpg");
  await page.locator(".verdict-cta").click();
  const share = page.locator(".clean-actions .btn-primary");
  await share.click();
  await expect(share).toHaveText("Share the clean copy");
  await expect(page.locator(".clean-actions .share-fallback")).toHaveCount(0);
  const download = page.waitForEvent("download");
  await page.locator(".clean-actions [data-share-save-fallback]").click();
  expect((await download).suggestedFilename()).toBe("photo-clean.jpg");
});

test("on a phone, an edited photo can be saved for sharing even without a file share sheet", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the fallback is for narrow screens");
  await page.setViewportSize({ width: 320, height: 568 });
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: () => false });
    Object.defineProperty(navigator, "share", { configurable: true, value: undefined });
  });
  await page.goto("./?sample=wifi.png");
  await page.getByRole("button", { name: "Black out the QR code…" }).click();
  await page.getByRole("button", { name: "Make the copy" }).click();
  const save = page.locator(".blackpicture .share-fallback");
  await expect(save).toHaveText("Save a copy to share from Files");
  await expect(page.locator(".blackpicture .clean-share-fallback")).toBeVisible();
  expect(await save.evaluate((button) => {
    const dialog = button.closest("dialog");
    const action = button.getBoundingClientRect();
    const frame = dialog?.getBoundingClientRect();
    return Boolean(frame && action.top >= frame.top && action.bottom <= frame.bottom);
  })).toBe(true);
  const downloadPromise = page.waitForEvent("download");
  await save.click();
  expect((await downloadPromise).suggestedFilename()).toBe("wifi-blacked-out.png");
  await page.getByRole("button", { name: "Open the copy" }).click();
  await expect(page.locator(".verdict-title")).toHaveText("Nothing personal found in this picture");
});

test("on a phone, the location badge shows the place in the summary, not the bytes", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "a computer's badge opens the bytes");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await page.locator("#location").click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "overview");
  await expect(page.locator(".reveal-list dt.is-flash")).toHaveText("Coordinates");
});

test("on a phone, several photos are cleaned with one button and shared with the next, in Ukrainian", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "sharing several copies at once is the phone's way");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    Object.defineProperty(navigator, "canShare", { value: () => true });
    Object.defineProperty(navigator, "share", { value: async () => undefined });
  });
  await page.goto("./");
  const photo = new URL("../public/samples/photo.jpg", import.meta.url).pathname;
  const iphone = new URL("./fixtures/iphone-hdr.jpg", import.meta.url).pathname;
  await page.locator("#picker-empty").setInputFiles([photo, iphone]);
  await expect(page.locator(".batch-title")).toHaveText("2 з 2 файлів варто переглянути");
  const make = page.locator(".batch-head .btn-clean");
  await expect(make).toHaveText("Зробити 2 чисті копії");
  await expect(page.locator(".batch-keep:not(.batch-fingerprints)")).toBeHidden();
  await make.click();
  const share = page.locator(".batch-actions .btn-primary");
  await expect(share).toHaveText("Поділитися 2 чистими копіями");
  // Sharing is now the one thing to do.
  await expect(page.locator(".batch-head .btn-clean")).toHaveCount(0);
  await expect(page.locator(".batch-head .btn-again")).toBeVisible();
  await expect(page.locator("#batch")).toContainText("прибрано 8 · залишилося 0 · не перевірено 0");
});

test("on a phone, a batch uses the share sheet when canShare is not implemented", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "sharing several files is a phone action");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: undefined });
    Object.defineProperty(navigator, "share", {
      configurable: true,
      value: async ({ files }: ShareData) => {
        const state = window as typeof window & { __sharedNames: string[] };
        state.__sharedNames = (files ?? []).map((file) => file.name);
      },
    });
  });
  await page.goto("./");
  const photo = new URL("../public/samples/photo.jpg", import.meta.url).pathname;
  const iphone = new URL("./fixtures/iphone-hdr.jpg", import.meta.url).pathname;
  await page.locator("#picker-empty").setInputFiles([photo, iphone]);
  await page.locator(".batch-head .btn-clean").click();
  const share = page.locator(".batch-actions .btn-primary");
  await expect(share).toHaveText("Share 2 clean copies");
  await share.click();
  const names = await page.evaluate(() => (window as typeof window & { __sharedNames: string[] }).__sharedNames);
  expect(names).toEqual(["photo-clean.jpg", "iphone-hdr-clean.jpg"]);
});

test("when a phone share sheet rejects a batch, the ZIP save route stays available", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the fallback is for narrow screens");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: undefined });
    Object.defineProperty(navigator, "share", {
      configurable: true,
      value: async () => { throw new DOMException("File sharing is unavailable", "NotAllowedError"); },
    });
  });
  await page.goto("./");
  const photo = new URL("../public/samples/photo.jpg", import.meta.url).pathname;
  const iphone = new URL("./fixtures/iphone-hdr.jpg", import.meta.url).pathname;
  await page.locator("#picker-empty").setInputFiles([photo, iphone]);
  await page.locator(".batch-head .btn-clean").click();
  const share = page.locator(".batch-actions .btn-primary");
  await expect(share).toHaveText("Share 2 clean copies");
  await share.click();
  await expect(page.locator(".batch-actions .btn-primary")).toHaveCount(0);
  await expect(page.locator(".batch-result")).toContainText("Sharing did not open. Use Save as a ZIP");
  const save = page.locator(".batch-result .btn").filter({ hasText: "Save as a ZIP" });
  await expect(save).toBeVisible();
  const download = page.waitForEvent("download");
  await save.click();
  expect((await download).suggestedFilename()).toBe("hexscope-clean-copies.zip");
});

test("when a phone share sheet is cancelled for a batch, its share and ZIP actions stay available", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the share sheet is a phone action");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    Object.defineProperty(navigator, "canShare", { configurable: true, value: undefined });
    Object.defineProperty(navigator, "share", {
      configurable: true,
      value: async () => { throw new DOMException("Share cancelled", "AbortError"); },
    });
  });
  await page.goto("./");
  const photo = new URL("../public/samples/photo.jpg", import.meta.url).pathname;
  const iphone = new URL("./fixtures/iphone-hdr.jpg", import.meta.url).pathname;
  await page.locator("#picker-empty").setInputFiles([photo, iphone]);
  await page.locator(".batch-head .btn-clean").click();
  const share = page.locator(".batch-actions .btn-primary");
  await expect(share).toHaveText("Share 2 clean copies");
  await share.click();
  await expect(share).toBeVisible();
  await expect(page.locator(".batch-result")).not.toContainText("Sharing did not open");
  const download = page.waitForEvent("download");
  await page.locator(".batch-result .btn").filter({ hasText: "Save as a ZIP" }).click();
  expect((await download).suggestedFilename()).toBe("hexscope-clean-copies.zip");
});
