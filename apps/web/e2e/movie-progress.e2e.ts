import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";

test("on a phone, a large video read shows progress and can be cancelled", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "large video reads are checked in the phone profile");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./");

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
    await page.locator("#picker-empty").setInputFiles(path);

    const cancel = page.getByRole("button", { name: "Cancel" });
    await expect(page.locator(".file-read-progress")).toContainText("Reading video", { timeout: 15_000 });
    await expect(cancel).toBeVisible();
    await expect(page.locator(".landing-menu")).toBeHidden();
    await cancel.click();
    await expect(page.locator("#load-error")).toHaveText("Reading was cancelled.");
    await expect(cancel).toHaveCount(0);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
