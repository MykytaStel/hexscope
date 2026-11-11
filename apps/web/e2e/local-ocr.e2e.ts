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
