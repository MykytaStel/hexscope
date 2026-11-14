import { expect, test } from "@playwright/test";
import { openView } from "./views";

test("phone header and sample categories use a quiet, compact rhythm", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "these density checks target the phone layout");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");

  const menu = page.locator(".landing-menu > summary");
  await expect(menu).toHaveAttribute("aria-label", "Меню");
  await expect(menu.locator("span")).toBeHidden();
  expect((await menu.boundingBox())?.height).toBeGreaterThanOrEqual(44);

  const more = page.locator("#guides");
  if (!(await more.evaluate((element) => (element as HTMLDetailsElement).open))) await more.locator(":scope > summary").click();
  const categories = page.locator(".sample-category");
  await expect(categories).toHaveCount(4);
  const rhythm = await categories.first().evaluate((element) => {
    const summary = element.querySelector(":scope > summary")!;
    const parent = element.parentElement!;
    const style = getComputedStyle(element);
    return {
      border: style.borderTopWidth,
      rowHeight: summary.getBoundingClientRect().height,
      gap: parseFloat(getComputedStyle(parent).rowGap),
    };
  });
  expect(rhythm.border, "mobile categories should read as a list, not four stacked cards").toBe("0px");
  expect(rhythm.rowHeight, "category row preserves its 44px touch target").toBe(44);
  expect(rhythm.gap, "adjacent categories use a compact rhythm").toBeLessThanOrEqual(4);
});

test("phone metadata surfaces privacy findings without repeating navigation", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "this spacing check targets the phone layout");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  await page.goto("./?sample=redacted.pdf");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await openView(page, "metadata");

  await expect(page.locator("#drawer .metadata-groups")).toBeVisible();
  const additional = page.locator("#drawer .metadata-additional-details");
  await expect(additional).toHaveCount(1);
  await additional.locator(":scope > summary").click();
  await expect(additional.locator(".metadata-priority-block")).toBeVisible();
  await expect(additional.locator(".metadata-priority-block")).toContainText("Olena Koval");
  const secondary = additional.locator(".metadata-secondary-fields");
  await expect(secondary.locator(":scope > summary")).toContainText("Інші поля");
  await expect(secondary).not.toHaveAttribute("open", "");
  const index = page.locator("#drawer .metadata-summary");
  await expect(index).toHaveCount(1);
  await expect(index).toBeHidden();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test("desktop metadata keeps its section index available", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "this check targets the desktop layout");
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  await page.goto("./?sample=report.docx");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await openView(page, "metadata");
  await expect(page.locator("#drawer .metadata-summary")).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
