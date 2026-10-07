import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { readFile } from "node:fs/promises";
import { crc32 } from "node:zlib";
import { DEFAULT_FILM_SETTINGS, filmRecipe } from "../src/film-lab";

async function openFilm(page: Page, orientation = 1): Promise<void> {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
  await page.goto("./");
  const bytes = await page.evaluate(async () => {
    const canvas = document.createElement("canvas");
    canvas.width = 400; canvas.height = 240;
    const g = canvas.getContext("2d")!;
    g.fillStyle = "rgb(230,170,100)";
    g.fillRect(0, 0, 400, 240);
    for (let x = 40; x < 360; x++) {
      const t = 0.12 + (x - 40) / 320 * 0.7;
      g.fillStyle = `rgb(${230 * t},${170 * t},${100 * t})`;
      g.fillRect(x, 24, 1, 192);
    }
    const blob = await new Promise<Blob>((ok) => canvas.toBlob((b) => ok(b!), "image/png"));
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  });
  let buffer = Buffer.from(bytes);
  if (orientation !== 1) {
    const tiff = Buffer.from([77, 77, 0, 42, 0, 0, 0, 8, 0, 1, 1, 18, 0, 3, 0, 0, 0, 1, 0, orientation, 0, 0, 0, 0, 0, 0]);
    const chunk = Buffer.concat([Buffer.from("eXIf"), tiff]);
    const length = Buffer.alloc(4), checksum = Buffer.alloc(4);
    length.writeUInt32BE(tiff.length); checksum.writeUInt32BE(crc32(chunk));
    buffer = Buffer.concat([buffer.subarray(0, 33), length, chunk, checksum, buffer.subarray(33)]);
  }
  await page.locator("#picker-empty").setInputFiles({ name: "private-roll.png", mimeType: "image/png", buffer });
  await page.locator(".film-lab-entry > summary").click();
  await expect(page.locator(".film-lab")).toHaveAttribute("data-state", "ready");
}

test("film lab crops, converts, reads the actual output and downloads locally while offline", async ({ page, context }) => {
  await openFilm(page);
  const lab = page.locator(".film-lab");
  await lab.getByLabel("Scan type").selectOption("color-negative");
  await lab.getByLabel("Film base red").fill("230");
  await lab.getByLabel("Film base green").fill("170");
  await lab.getByLabel("Film base blue").fill("100");
  for (const edge of ["left", "top", "right", "bottom"]) await lab.getByLabel(`Trim ${edge} (%)`, { exact: true }).fill("10");
  await expect(lab).toHaveAttribute("data-state", "ready");
  await expect(lab.locator(".film-lab-size")).toHaveText("320 × 192");
  const urls = await lab.locator("img").evaluateAll((images) => images.map((i) => (i as HTMLImageElement).src));
  expect(urls[0]).not.toBe(urls[1]);
  await context.setOffline(true);
  await lab.getByRole("button", { name: "Create JPEG copy", exact: true }).click();
  await expect(lab.locator(".film-lab-check")).toContainText("No metadata findings detected");
  await expect(lab.locator(".film-lab-check")).toContainText("No QR codes detected");
  const downloadPromise = page.waitForEvent("download");
  await lab.getByRole("button", { name: "Download JPEG", exact: true }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("film-copy.jpg");
  const bytes = Array.from(await readFile((await download.path())!));
  const dimensions = await page.evaluate(async (b) => {
    const bitmap = await createImageBitmap(new Blob([new Uint8Array(b)], { type: "image/jpeg" }));
    const result = [bitmap.width, bitmap.height]; bitmap.close(); return result;
  }, bytes);
  expect(dimensions).toEqual([320, 192]);
  await expect(page.locator(".filename").first()).toContainText("private-roll.png");
});

test("recipes validate before applying and film controls work in Ukrainian with accessible labels", async ({ page }) => {
  await openFilm(page);
  const lab = page.locator(".film-lab");
  await lab.locator(".film-recipe-input").setInputFiles({ name: "bad.json", mimeType: "application/json", buffer: Buffer.from('{"version":99}') });
  await expect(lab.locator(".film-lab-message")).toContainText("Could not load this recipe");
  const recipe = filmRecipe({ ...DEFAULT_FILM_SETTINGS, mode: "mono-negative", crop: [0.1, 0.1, 0.1, 0.1] });
  await lab.locator(".film-recipe-input").setInputFiles({ name: "roll.json", mimeType: "application/json", buffer: Buffer.from(recipe) });
  await expect(lab.getByLabel("Scan type")).toHaveValue("mono-negative");
  await expect(lab).toHaveAttribute("data-state", "ready");
  await expect(lab.locator(".film-lab-size")).toHaveText("320 × 192");
  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await expect(lab.getByLabel("Тип скану")).toHaveValue("mono-negative");
  await expect(lab.getByRole("button", { name: "Створити JPEG-копію", exact: true })).toBeVisible();
  const selectStyle = await lab.getByLabel("Тип скану").evaluate((element) => {
    const computed = getComputedStyle(element);
    return { backgroundImage: computed.backgroundImage, paddingRight: parseFloat(computed.paddingRight) };
  });
  expect(selectStyle.backgroundImage).toContain("linear-gradient");
  expect(selectStyle.paddingRight).toBeGreaterThanOrEqual(36);
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
  const accessibility = await new AxeBuilder({ page }).include(".film-lab").analyze();
  expect(accessibility.violations).toEqual([]);
});

test("film lab re-reads metadata-bearing JPEG output and releases its view on another file", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".tour")).toBeVisible();
  await page.locator(".tour").getByRole("button", { name: "Skip", exact: true }).click();
  await expect(page.locator(".tour")).toHaveCount(0);
  await page.locator(".film-lab-entry > summary").click();
  const lab = page.locator(".film-lab");
  await expect(lab).toHaveAttribute("data-state", "ready");
  await lab.getByRole("button", { name: "Create JPEG copy", exact: true }).click();
  await expect(lab.locator(".film-lab-check")).toContainText("No metadata findings detected");
  await expect(page.locator(".reveals")).toContainText("Camera");
  await page.locator("#picker").setInputFiles({ name: "next.txt", mimeType: "text/plain", buffer: Buffer.from("another file") });
  await expect(lab).toHaveCount(0);
});

test("reset replaces an in-flight picked film base instead of replaying the old sample", async ({ page }) => {
  await page.addInitScript(() => {
    const send = Worker.prototype.postMessage as (message: unknown, options?: Transferable[] | StructuredSerializeOptions) => void;
    Worker.prototype.postMessage = function (message, ...args) {
      if (message?.type === "filmRender" && message.point) setTimeout(() => send.call(this, message, ...args), 500);
      else send.call(this, message, ...args);
    };
  });
  await openFilm(page);
  const lab = page.locator(".film-lab");
  await lab.getByLabel("Scan type").selectOption("color-negative");
  await expect(lab).toHaveAttribute("data-state", "ready");
  await lab.getByRole("button", { name: "Pick unexposed film" }).click();
  await lab.locator(".film-lab-source").click({ position: { x: 70, y: 40 } });
  await expect(lab).toHaveAttribute("data-state", "loading");
  // Let the debounced picked request enter the worker queue, then reset its intent.
  await page.waitForTimeout(220);
  await lab.getByRole("button", { name: "Reset adjustments" }).click();
  await expect(lab).toHaveAttribute("data-state", "ready");
  await lab.getByLabel("Scan type").selectOption("color-negative");
  await expect(lab).toHaveAttribute("data-state", "ready");
  await expect(lab.getByLabel("Film base red")).toHaveValue("230");
  await expect(lab.getByLabel("Film base green")).toHaveValue("170");
  await expect(lab.getByLabel("Film base blue")).toHaveValue("100");
});

test("rotated PNG scans retain their geometry and film base sampling works with keyboard controls", async ({ page }) => {
  await openFilm(page, 6);
  const lab = page.locator(".film-lab");
  await expect(lab.locator(".film-lab-size")).toHaveText("240 × 400");
  await lab.getByLabel("Scan type").selectOption("color-negative");
  await lab.getByText("Sample by coordinates", { exact: true }).click();
  await lab.getByLabel("Sample x (%)").fill("5");
  await lab.getByLabel("Sample y (%)").fill("5");
  await lab.getByRole("button", { name: "Sample film base", exact: true }).click();
  await expect(lab).toHaveAttribute("data-state", "ready");
  await expect(lab.getByLabel("Film base red")).toHaveValue("230");
  await lab.getByRole("button", { name: "Create JPEG copy", exact: true }).click();
  await expect(lab.locator(".film-lab-check")).toContainText("No metadata findings detected");
  await expect(lab.locator(".film-lab-size")).toHaveText("240 × 400");
});

test("the landing discloses its synthetic detector illustration before opening it", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./");
  const example = page.locator(".film-synthetic-example");
  await example.locator(":scope > summary").click();
  await expect(example).toContainText("it is not a photograph");
  await example.getByRole("button", { name: "Try a synthetic film negative" }).click();
  await expect(page.locator(".film-scan-card")).toContainText("Frame-like boundary");
  await page.locator(".film-lab-entry > summary").click();
  const lab = page.locator(".film-lab");
  await expect(lab).toHaveAttribute("data-state", "ready");
  await lab.getByLabel("Scan type").selectOption("color-negative");
  await expect(lab).toHaveAttribute("data-state", "ready");
  await expect(lab.getByRole("button", { name: "Use detected frame" })).toBeVisible();
  await lab.getByRole("button", { name: "Use detected frame" }).click();
  await expect(lab).toHaveAttribute("data-state", "ready");
  const size = (await lab.locator(".film-lab-size").textContent())!.split(" × ").map(Number);
  expect(size[0]).toBeLessThan(640); expect(size[1]).toBeLessThan(440);
});
