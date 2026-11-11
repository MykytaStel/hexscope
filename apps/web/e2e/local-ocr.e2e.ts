import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";

test("local OCR suggests words, adds only a confirmed box, and supports undo/redo", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "bounded OCR experiment uses one desktop engine");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en"); localStorage.setItem("hexscope.tour", "done");
  });
  const csp = readFileSync(new URL("../public/_headers", import.meta.url), "utf8").match(/Content-Security-Policy: (.+)/)![1];
  await page.route("**/", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, headers: { ...response.headers(), "content-security-policy": csp } });
  });
  const leaked: string[] = [];
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (/https?:/.test(url.protocol) && !["localhost", "127.0.0.1"].includes(url.hostname)) leaked.push(request.url());
  });
  await page.goto("./");
  const source = await page.evaluate(() => {
    const canvas = document.createElement("canvas"); canvas.width = 900; canvas.height = 220;
    const ctx = canvas.getContext("2d")!; ctx.fillStyle = "white"; ctx.fillRect(0, 0, 900, 220);
    ctx.fillStyle = "black"; ctx.font = "44px sans-serif";
    ctx.fillText("PRIVATE EMAIL", 40, 80); ctx.fillText("alice@example.com", 40, 155);
    return canvas.toDataURL().split(",")[1];
  });
  await page.locator("#picker-empty").setInputFiles({ name: "ocr-sample.png", mimeType: "image/png", buffer: Buffer.from(source, "base64") });
  await page.locator(".blackout-open").click();
  const editor = page.locator("dialog.blackpicture");
  await editor.locator(".local-ocr summary").click();
  await editor.getByRole("button", { name: "Find text locally" }).click();
  await expect(editor.locator(".local-ocr")).toContainText("Suggested words:", { timeout: 60_000 });
  await expect(editor.locator(".local-ocr")).toContainText("alice");
  await expect(editor.locator(".blackpicture-boxes li")).toHaveCount(0);
  await editor.getByRole("button", { name: "Add checked box", exact: true }).first().click();
  await expect(editor.locator(".blackpicture-boxes li")).toHaveCount(1);
  await editor.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(editor.locator(".blackpicture-boxes li")).toHaveCount(0);
  await editor.getByRole("button", { name: "Redo", exact: true }).click();
  await expect(editor.locator(".blackpicture-boxes li")).toHaveCount(1);
  expect(leaked).toEqual([]);
});


test("OCR cancellation releases its worker while a language model is still loading", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "one bounded desktop OCR lifecycle check");
  await page.addInitScript(() => { localStorage.setItem("hexscope.language", "en"); localStorage.setItem("hexscope.tour", "done"); });
  let release: () => void = () => {};
  const blocked = new Promise<void>((resolve) => { release = resolve; });
  await page.route("**/*.traineddata.gz", async (route) => { await blocked; await route.abort().catch(() => {}); });
  await page.goto("./?sample=photo.jpg");
  await page.locator(".blackout-open").click();
  const editor = page.locator("dialog.blackpicture");
  await editor.locator(".local-ocr summary").click();
  const model = page.waitForRequest(/traineddata\.gz$/);
  await editor.getByRole("button", { name: "Find text locally" }).click();
  try {
    await model;
    await editor.getByRole("button", { name: "Cancel recognition" }).click();
    await expect(editor.locator(".local-ocr")).toContainText("OCR cancelled");
    await expect.poll(() => page.workers().filter((worker) => /ocr-worker-|worker.min.js/.test(worker.url())).length).toBe(0);
  } finally { release(); }
});

test("Ukrainian OCR on a PDF scan works with the unchanged content policy", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "one bounded desktop OCR test per source kind");
  await page.addInitScript(() => { localStorage.setItem("hexscope.language", "en"); localStorage.setItem("hexscope.tour", "done"); });
  await page.goto("./");
  const images = await page.evaluate(() => {
    const canvas = document.createElement("canvas"); canvas.width = 900; canvas.height = 220;
    const ctx = canvas.getContext("2d")!; ctx.fillStyle = "white"; ctx.fillRect(0, 0, 900, 220);
    const blank = canvas.toDataURL("image/jpeg", 0.95).split(",")[1];
    ctx.fillStyle = "black"; ctx.font = "56px Arial"; ctx.fillText("ПРИВАТНІ ДАНІ", 40, 130);
    return { blank, scan: canvas.toDataURL("image/jpeg", 0.95).split(",")[1] };
  });
  const pdf = scannedPdf(Buffer.from(images.blank, "base64"), Buffer.from(images.scan, "base64"));
  await page.locator("#picker-empty").setInputFiles({ name: "synthetic-ukrainian-scan.pdf", mimeType: "application/pdf", buffer: pdf });
  await page.getByRole("button", { name: "Draw a box on a page instead" }).click();
  await page.getByText("Experimental image OCR", { exact: true }).first().click();
  const panel = page.locator(".pdf-ocr-tools").first();
  await panel.getByLabel("Page image").selectOption("1");
  await panel.getByRole("button", { name: "Show selected image on page" }).click();
  await expect(page.locator(".redact-preview .redact-drawing").first()).toHaveAttribute("x", "375");
  await panel.getByLabel("OCR language").selectOption("ukr");
  const model = page.waitForRequest(/\/ukr\.traineddata\.gz$/);
  await panel.getByRole("button", { name: "Recognize the selected image locally" }).click();
  await model;
  await panel.getByRole("button", { name: "ДАНІ", exact: true }).click();
  await expect(page.locator(".redact-preview .redact-drawing").first()).toBeVisible();
  await panel.getByRole("button", { name: "Add checked area: ДАНІ", exact: true }).click();
  await expect(page.locator(".redactor .btn-primary")).toBeVisible();
});

/** Synthetic one-page scan, no user text or files in the fixture. */
function scannedPdf(blank: Buffer, jpeg: Buffer): Buffer {
  const stream = (bytes: Buffer, attrs = "") => Buffer.concat([Buffer.from(`<< ${attrs} /Length ${bytes.length} >>\nstream\n`), bytes, Buffer.from("\nendstream")]);
  const objects = [
    Buffer.from("<< /Type /Catalog /Pages 2 0 R >>"),
    Buffer.from("<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
    Buffer.from("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 300] /Resources << /XObject << /Im0 4 0 R /Im1 6 0 R >> >> /Contents 5 0 R >>"),
    stream(blank, "/Type /XObject /Subtype /Image /Width 900 /Height 220 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode"),
    stream(Buffer.from("q 300 0 0 74 0 110 cm /Im0 Do Q q 0 250 -61 0 436 20 cm /Im1 Do Q\n")),
    stream(jpeg, "/Type /XObject /Subtype /Image /Width 900 /Height 220 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode"),
  ];
  const parts = [Buffer.from("%PDF-1.7\n")]; const offsets: number[] = [];
  let length = parts[0].length;
  objects.forEach((object, index) => {
    offsets.push(length);
    const next = Buffer.concat([Buffer.from(`${index + 1} 0 obj\n`), object, Buffer.from("\nendobj\n")]);
    parts.push(next); length += next.length;
  });
  parts.push(Buffer.from(`xref\n0 7\n0000000000 65535 f \n${offsets.map((offset) => `${String(offset).padStart(10, "0")} 00000 n \n`).join("")}trailer\n<< /Root 1 0 R /Size 7 >>\nstartxref\n${length}\n%%EOF\n`));
  return Buffer.concat(parts);
}
