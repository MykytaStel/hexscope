import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { DEFAULT_FILM_SETTINGS, filmRecipe } from "../src/film-lab";
function storedEntries(bytes: Buffer): Map<string, Buffer> {
  const entries = new Map<string, Buffer>(); let at = 0;
  while (bytes.readUInt32LE(at) === 0x04034b50) {
    const size = bytes.readUInt32LE(at + 18), n = bytes.readUInt16LE(at + 26), extra = bytes.readUInt16LE(at + 28);
    const name = bytes.subarray(at + 30, at + 30 + n).toString(); const start = at + 30 + n + extra;
    entries.set(name, bytes.subarray(start, start + size)); at = start + size;
  }
  return entries;
}
function tiffSamples(bytes: Buffer): number[] {
  const le = bytes.toString("ascii", 0, 2) === "II";
  const u16 = (at: number) => le ? bytes.readUInt16LE(at) : bytes.readUInt16BE(at);
  const u32 = (at: number) => le ? bytes.readUInt32LE(at) : bytes.readUInt32BE(at);
  const ifd = u32(4), count = u16(ifd); const tags = new Map<number, number>();
  for (let i = 0; i < count; i++) { const at = ifd + 2 + 12 * i; tags.set(u16(at), u32(at + 8)); }
  expect(tags.get(256)).toBe(2); expect(tags.get(257)).toBe(2);
  const bits = tags.get(258)!; expect([u16(bits),u16(bits+2),u16(bits+4)]).toEqual([16,16,16]);
  const strip = tags.get(273)!; return Array.from({ length: 12 }, (_, i) => u16(strip + i * 2));
}

test("roll uses the shared renderer, preserves TIFF16 precision and exports a receipt", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
  const requests: string[] = []; page.on("request", (r) => requests.push(r.url()));
  await page.goto("./");
  expect(requests.some((url) => url.includes("hexscope_raster"))).toBe(false);
  await page.locator("#film-roll-open").click();
  const roll = page.locator(".film-roll");
  await roll.getByLabel("Choose scans", { exact: true }).setInputFiles("../../crates/hexscope-raster/tests/fixtures/precision16.tif");
  await roll.locator("details > summary").click();
  await roll.getByLabel("Roll recipe", { exact: true }).fill(filmRecipe(DEFAULT_FILM_SETTINGS));
  await roll.getByLabel("Output format").selectOption("tiff16");
  await roll.getByRole("button", { name: "Process roll", exact: true }).click();
  await expect(roll.getByRole("status")).toContainText("1 / 1 copies ready");
  await expect(roll.getByRole("status")).toContainText("not certified safe");
  const download = page.waitForEvent("download"); await roll.getByRole("button", { name: "Download roll ZIP" }).click();
  const archive = storedEntries(await readFile((await (await download).path())!));
  const copy = archive.get("000001-film.tif")!;
  expect(tiffSamples(copy)).toEqual([257,300,32001,50000,65400,65535,400,500,600,700,800,900]);
  const report = JSON.parse(archive.get("hexscope-film-report.json")!.toString());
  expect(report).toMatchObject({ written: 1, failed: 0, files: [{ sourceDepth: 16, outputDepth: 16, sha256: createHash("sha256").update(copy).digest("hex") }] });
  expect(JSON.stringify(report)).not.toContain("precision16");
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});

test("batch reports readback and shows local correlations without exporting personal values", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en")); await page.goto("./");
  const photo = await readFile("../../crates/hexscope-core/tests/fixtures/photo.png");
  await page.locator("#picker-empty").setInputFiles([{ name: "a.png", mimeType: "image/png", buffer: photo }, { name: "b.png", mimeType: "image/png", buffer: photo }, { name: "unknown.bin", mimeType: "application/octet-stream", buffer: Buffer.from("unknown") }]);
  await expect(page.locator("#batch")).toContainText("Photo privacy mosaic");
  await expect(page.locator("#batch")).not.toContainText("Every one can be sent");
  const zipDownload = page.waitForEvent("download");
  await page.locator("#batch").getByRole("button", { name: /Save 2 clean copies/ }).click();
  const zip = storedEntries(await readFile((await (await zipDownload).path())!));
  const receipt = JSON.parse(zip.get("hexscope-report.json")!.toString());
  expect(receipt).toMatchObject({ written: 2, failed: 0, removed: 8 }); expect(receipt.unchecked).toBeGreaterThan(0);
  await page.getByRole("button", { name: "Photo privacy mosaic", exact: true }).click();
  const mosaic = page.locator(".photo-mosaic"); await expect(mosaic.getByRole("status")).toContainText("2 / 3 files checked");
  await expect(mosaic).toContainText("1: a.png → 2: b.png");
  const jsonDownload = page.waitForEvent("download"); await mosaic.getByRole("button", { name: "Save mosaic report" }).click();
  const text = await readFile((await (await jsonDownload).path())!, "utf8"), report = JSON.parse(text);
  expect(report.serial_groups).toEqual([[1,2]]); expect(report.not_checked_indexes).toEqual([3]);
  expect(text).not.toMatch(/HX-000042|a\.png|b\.png|2024:/);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});
