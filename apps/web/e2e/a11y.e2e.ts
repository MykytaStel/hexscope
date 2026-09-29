// Accessibility, checked by axe on every screen people reach, in both
// themes, on a computer and a phone: contrast, names, roles, structure.
import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

async function check(page: Page, where: string): Promise<void> {
  const r = await new AxeBuilder({ page }).analyze();
  const found = r.violations.map(
    (v) => `${v.id}: ${v.nodes.slice(0, 3).map((n) => `${n.target.join(" ")} (${(n.any[0]?.message ?? "").replace(/Expected contrast ratio of 4.5:1|Element has insufficient color contrast of /g, "")})`).join(" | ")}`,
  );
  expect(found, where).toEqual([]);
}

for (const scheme of ["dark", "light"] as const) {
  test.describe(`${scheme} theme`, () => {
    // Checked at rest: a line fading in is measured once it is in.
    test.use({ colorScheme: scheme, reducedMotion: "reduce" });

    test("the landing page", async ({ page }) => {
      await page.goto("./");
      await page.waitForFunction(() => document.body.dataset.state === "empty");
      // Past the demonstration's first scene: its facts drawn.
      await page.waitForTimeout(1500);
      await check(page, "landing");
    });

    for (const sample of ["photo.jpg", "redacted.pdf", "phishing.eml", "phishing.msg", "plan.doc", "budget.xlsx", "broken.png"]) {
      test(`a file: ${sample}`, async ({ page }) => {
        page.on("download", (d) => void d.cancel());
        await page.goto(`./?sample=${sample}`);
        await expect(page.locator(".verdict-title")).toBeVisible();
        await check(page, sample);
        // And after its clean copy, where there is one.
        const cta = page.locator(".verdict-cta");
        if (await cta.count()) {
          await cta.click();
          await expect(page.locator(".before-after, .cleaner .problem")).toBeVisible();
          await check(page, `${sample}, clean copy made`);
        }
      });
    }

    test("the privacy page", async ({ page }) => {
      await page.goto("./your-files-stay-private");
      await expect(page.locator("h1")).toHaveText("Your files stay on your device");
      await check(page, "privacy");
    });

    test("the keyboard list", async ({ page }, info) => {
      test.skip(info.project.name !== "computer", "keys are for a computer");
      await page.goto("./");
      await page.waitForFunction(() => document.body.dataset.state === "empty");
      await page.keyboard.press("?");
      await expect(page.locator("dialog.shortcuts")).toBeVisible();
      await check(page, "keyboard list");
    });
  });
}
