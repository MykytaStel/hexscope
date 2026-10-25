import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";

test("on a phone, a large video read shows progress and can be cancelled", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "large video reads are checked in the phone profile");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
    (window as Window & { __hexscopeProgressEvents?: number }).__hexscopeProgressEvents = 0;
    const NativeWorker = window.Worker;
    window.Worker = class ProgressWorker extends NativeWorker {
      override addEventListener(type: string, listener: EventListenerOrEventListenerObject, options?: boolean | AddEventListenerOptions): void {
        if (type !== "message") {
          super.addEventListener(type, listener, options);
          return;
        }
        let firstProgress = true;
        let timer = 0;
        let deliveredQueuedMessage = false;
        const queued: MessageEvent[] = [];
        const deliver = (event: MessageEvent) => {
          const data = event.data as { type?: string; progress?: { cancellable?: boolean } };
          if (data.type === "progress" && data.progress?.cancellable) {
            const state = window as Window & { __hexscopeProgressEvents?: number };
            state.__hexscopeProgressEvents = (state.__hexscopeProgressEvents ?? 0) + 1;
          }
          if (typeof listener === "function") listener.call(this, event);
          else listener.handleEvent(event);
        };
        const pump = () => {
          while (!deliveredQueuedMessage && queued.length) {
            const event = queued.shift()!;
            const data = event.data as { type?: string; progress?: { stage?: string; loaded?: number; total?: number; cancellable?: boolean } };
            const progress = data.progress;
            if (data.type !== "progress" || !progress?.cancellable) continue;
            deliveredQueuedMessage = true;
            deliver(event);
          }
          timer = 0;
        };
        const wrapped = (event: MessageEvent) => {
          const data = event.data as { type?: string; progress?: { cancellable?: boolean } };
          if (data.type === "progress" && data.progress?.cancellable) {
            if (firstProgress) {
              firstProgress = false;
              deliver(event);
            } else {
              queued.push(event);
              if (!timer) timer = window.setTimeout(pump, 100);
            }
          } else if ((data.type === "progress" || data.type === "parsed") && queued.length) {
            queued.push(event);
            if (!timer) timer = window.setTimeout(pump, 100);
          } else {
            deliver(event);
          }
        };
        super.addEventListener(type, wrapped as EventListener, options);
      }
    };
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toContainText("This photo");

  const bytes = Buffer.alloc(64 * 1024 * 1024 + 20);
  bytes.writeUInt32BE(12, 0);
  bytes.write("ftyp", 4, 4, "ascii");
  bytes.write("isom", 8, 4, "ascii");
  bytes.writeUInt32BE(bytes.length - 12, 12);
  bytes.write("mdat", 16, 4, "ascii");
  const directory = mkdtempSync(join(tmpdir(), "hexscope-movie-progress-"));
  const path = join(directory, "large.mov");
  try {
    writeFileSync(path, bytes);
    await page.locator("#picker").setInputFiles(path);

    const cancel = page.getByRole("button", { name: "Cancel" });
    const progress = page.locator(".file-read-progress");
    await expect(progress).toContainText("Reading video", { timeout: 15_000 });
    await expect(progress).toHaveRole("progressbar");
    await expect(progress).toHaveAttribute("aria-label", "Video read progress");
    await expect(progress).toHaveAttribute("aria-valuenow", /\d+/);
    const announcement = page.locator(".file-read-announcement");
    await expect(announcement).toHaveAttribute("aria-live", "polite");
    await expect(announcement).toContainText("Reading video");
    await expect(cancel).toBeVisible();
    await expect(page.locator(".landing-menu")).toBeHidden();
    await cancel.focus();
    await expect.poll(() => page.evaluate(() =>
      (window as Window & { __hexscopeProgressEvents?: number }).__hexscopeProgressEvents ?? 0,
    ), { timeout: 8_000 }).toBeGreaterThan(1);
    await expect(cancel).toBeEnabled();
    await expect(cancel).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(cancel).toHaveCount(0);
    await expect(page.locator(".fileinfo .filename")).toHaveText("photo.jpg");
    await expect(page.locator(".fileinfo .file-read-announcement")).toHaveText("Reading was cancelled.");
    await expect(page.locator(".verdict-title")).toContainText("This photo");
    await page.locator(".verdict-cta").click();
    const after = page.locator(".ba-side.is-after");
    await expect(after.locator(".ba-count")).toHaveText("8");
    await expect(after.locator(".ba-what")).toHaveText("removed — nothing is left in the copy");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
