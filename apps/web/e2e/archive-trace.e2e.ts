import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { openView } from "./views";

async function openTrace(page: import("@playwright/test").Page, language: "en" | "uk" = "en") {
  await page.addInitScript((lang) => {
    localStorage.setItem("hexscope.language", lang);
    localStorage.setItem("hexscope.tour", "done");
  }, language);
  await page.goto("./?sample=report.docx");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await openView(page, "structure");
  await page.getByRole("treeitem").nth(1).click();

  const inspector = page.locator(".inspector-sheet");
  if (await inspector.isVisible()) {
    const details = inspector.getByRole("button", { name: language === "uk" ? "Деталі" : "Details" });
    if (await details.isVisible()) await details.click();
  }
  const trace = page.getByRole("group", { name: language === "uk" ? "Шлях запису в архіві" : "Archive Trace" });
  await expect(trace.getByRole("button", { name: language === "uk" ? /запису центрального каталогу/ : "Show central-directory record bytes" })).toBeVisible();
  return trace;
}

test("ZIP Trace loads only when a ZIP entry is selected", async ({ page }) => {
  const traceChunks: string[] = [];
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (/\/assets\/archive-trace-view-[^/]+\.js$/.test(url.pathname)) traceChunks.push(url.href);
  });
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));

  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  expect(traceChunks).toHaveLength(0);

  await page.goto("./?sample=report.docx");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await openView(page, "structure");
  await page.getByRole("treeitem").nth(1).click();
  const inspector = page.locator(".inspector-sheet");
  if (await inspector.isVisible()) {
    const details = inspector.getByRole("button", { name: "Details" });
    if (await details.isVisible()) await details.click();
  }
  await expect(page.getByRole("group", { name: "Archive Trace" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Show central-directory record bytes" })).toBeVisible();
  expect(traceChunks).toHaveLength(1);
});

test("Archive Trace selects each exact range and reuses the DEFLATE player", async ({ page }) => {
  const trace = await openTrace(page);
  await expect(trace.locator("ol > li")).toHaveCount(4);
  const central = trace.getByRole("button", { name: "Show central-directory record bytes" });
  const local = trace.getByRole("button", { name: "Show local header bytes" });
  const compressed = trace.getByRole("button", { name: "Show compressed data bytes" });

  await central.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  await expect(page.locator(".hex-scroller")).toBeFocused();

  await local.click();
  await compressed.click();
  await expect(trace).toContainText("Available bytes");
  await expect(trace).toContainText("Declared bytes");

  for (const width of [320, 390, 768, 1280]) {
    await page.setViewportSize({ width, height: 840 });
    await expect(page.locator("html")).toHaveJSProperty("scrollWidth", width);
  }

  const accessibility = await new AxeBuilder({ page }).include(".archive-trace").analyze();
  expect(accessibility.violations).toEqual([]);

  await trace.getByRole("button", { name: "Watch it decompress", exact: true }).click();
  await expect(page.locator(".drawer-player")).toBeVisible();
});

test.describe("player chunk retry", () => {
  test.use({ serviceWorkers: "block" });

  test("keeps the ZIP entry after Escape clears its selection", async ({ page }) => {
    let requests = 0;
    await page.route("**/player-*.js", async (route) => {
      requests++;
      if (requests === 1) await route.abort();
      else await route.continue();
    });

    const trace = await openTrace(page);
    await trace.getByRole("button", { name: "Watch it decompress", exact: true }).click();
    await expect.poll(() => requests).toBe(1);
    const error = page.locator("#player-load-error");
    await expect(error).toBeVisible();

    await page.keyboard.press("Escape");
    await error.locator("[data-retry-player]").click();

    await expect.poll(() => requests).toBe(2);
    await expect(page.locator(".drawer-player")).toBeVisible();
  });
});

test("Archive Trace labels and byte controls are localized in Ukrainian", async ({ page }) => {
  const trace = await openTrace(page, "uk");
  await expect(trace.getByRole("heading", { name: "Запис центрального каталогу" })).toBeVisible();
  await expect(trace.getByRole("heading", { name: "Локальний заголовок" })).toBeVisible();
  await expect(trace.getByRole("heading", { name: "Стиснені дані" })).toBeVisible();
  await expect(trace.getByRole("heading", { name: "Розпакований вміст" })).toBeVisible();
  await expect(trace.getByRole("button", { name: "Показати байти запису центрального каталогу" })).toBeVisible();
  await expect(trace.locator(".archive-trace-byte-count").first()).toHaveText(/^\d[\d,]* байт(и|ів)?$/);
});
