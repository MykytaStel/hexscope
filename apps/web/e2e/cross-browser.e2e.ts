import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
});

test("reads the photo sample and names its privacy findings", async ({ page }) => {
  await page.goto("./?sample=photo.jpg");

  await expect(page.getByRole("heading", { name: "This photo gives away 8 things" })).toBeVisible();
  await expect(page.locator(".reveal-list")).toContainText("Location");
  await expect(page.locator(".reveal-list")).toContainText("Serial number");
});

test("the clean-copy action works from the keyboard and verifies the output", async ({ page }) => {
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toContainText("This photo gives away 8 things");

  const action = page.locator(".verdict-cta");
  await action.focus();
  await expect(action).toBeFocused();
  await page.keyboard.press("Enter");

  const after = page.locator(".ba-side.is-after");
  await expect(after.locator(".ba-count")).toHaveText("8");
  await expect(after.locator(".ba-what")).toHaveText("confirmed removed from the copy");
  await expect(after).toHaveClass(/is-clear/);
});
