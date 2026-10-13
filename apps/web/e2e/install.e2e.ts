// "Install the app": where the browser offers it, the button asks the
// browser; an iPhone, which has no such offer, is told the three taps.
import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
});

test("where the browser offers to install, the button asks it, and goes once installed", async ({ page }) => {
  await page.goto("./");
  const install = page.locator(".footnote-links [data-install]");
  await expect(install).toBeHidden();
  await page.evaluate(() => {
    const offer = new Event("beforeinstallprompt", { cancelable: true });
    Object.assign(offer, {
      prompt: async () => document.body.setAttribute("data-asked", "yes"),
      userChoice: Promise.resolve({ outcome: "accepted" }),
    });
    dispatchEvent(offer);
  });
  await expect(install).toBeVisible();
  await install.click();
  await expect(page.locator("body")).toHaveAttribute("data-asked", "yes");
  await expect(install).toBeHidden();
});

test.describe("on an iPhone", () => {
  test.use({
    userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
  });

  test("it is told the three taps that install hexscope", async ({ page }) => {
    await page.goto("./");
    const install = page.locator(".footnote-links [data-install]");
    await expect(install).toBeVisible();
    await install.click();
    const steps = page.locator("dialog.install-steps");
    await expect(steps.locator("h2")).toHaveText("Install hexscope on this iPhone");
    await expect(steps.locator("li")).toHaveCount(3);
    await expect(steps).toContainText("Add to Home Screen");
    await steps.getByRole("button", { name: "Got it" }).click();
    await expect(steps).toHaveCount(0);
  });
});
