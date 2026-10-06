// Accessibility, checked by axe on every screen people reach, in both
// themes, on a computer and a phone: contrast, names, roles, structure.
import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";

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

    test.beforeEach(async ({ page }) => {
      await page.addInitScript((theme) => {
        if (location.origin !== "null") localStorage.setItem("hexscope.theme", theme);
      }, scheme);
    });

    test("the landing page", async ({ page }) => {
      await page.goto("./");
      await page.waitForFunction(() => document.body.dataset.state === "empty");
      // Past the demonstration's first scene: its facts drawn.
      await page.waitForTimeout(1500);
      await check(page, "landing");
    });

    test("shared controls keep clear focus and disabled states", async ({ page }) => {
      await page.goto("./");
      await page.waitForFunction(() => document.body.dataset.state === "empty");
      const chooseFile = page.getByRole("button", { name: "Choose a file" });
      const bounds = await chooseFile.boundingBox();
      expect(bounds?.height).toBeGreaterThanOrEqual(44);

      await page.locator("#film-roll-open").click();
      const dialog = page.locator("dialog.film-roll");
      const process = dialog.getByRole("button", { name: "Process roll" });
      const close = dialog.getByRole("button", { name: "Close film roll" });
      await expect(process).toBeDisabled();
      const [disabled, secondary] = await Promise.all([process, close].map((control) => control.evaluate((element) => {
        const style = getComputedStyle(element);
        return { background: style.backgroundColor, border: style.borderColor, color: style.color };
      })));
      const accent = await process.evaluate(() => {
        const probe = document.createElement("span");
        probe.style.backgroundColor = "var(--accent)";
        document.body.append(probe);
        const color = getComputedStyle(probe).backgroundColor;
        probe.remove();
        return color;
      });
      expect(disabled.background).not.toBe(accent);
      expect(disabled.background).not.toBe(secondary.background);
      expect(disabled.border).not.toBe(secondary.border);

      // Enter keyboard modality, then check both native disclosure and select.
      await page.keyboard.press("Tab");
      const summary = dialog.locator(".film-roll-advanced > summary");
      const select = dialog.getByRole("combobox", { name: "Scan type" });
      for (const control of [summary, select]) {
        await control.focus();
        await expect(control).toBeFocused();
        expect(await control.evaluate((element) => element.matches(":focus-visible"))).toBe(true);
        expect(await control.evaluate((element) => getComputedStyle(element).outlineWidth)).not.toBe("0px");
      }
    });

    test("film roll keeps controls and actions inside the dialog at narrow widths", async ({ page }) => {
      await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
      await page.goto("./");
      await page.locator("#film-roll-open").click();
      const dialog = page.locator("dialog.film-roll");
      await expect(dialog).toBeVisible();
      await expect(dialog.getByLabel("Тип скану")).toHaveValue("positive");
      await expect(dialog.getByLabel("Тип скану").locator("option:checked")).toHaveText("Позитив (проявлений)");

      const selectStyles = await dialog.locator(".film-roll-field select").evaluateAll((elements) => elements.map((element) => {
        const computed = getComputedStyle(element);
        return { backgroundImage: computed.backgroundImage, paddingRight: parseFloat(computed.paddingRight) };
      }));
      for (const style of selectStyles) {
        expect(style.backgroundImage).toContain("linear-gradient");
        expect(style.paddingRight).toBeGreaterThanOrEqual(36);
      }

      for (const width of [320, 360, 600, 768, 1024, 1440]) {
        await page.setViewportSize({ width, height: 820 });
        const bounds = await dialog.evaluate((element) => {
          const rect = element.getBoundingClientRect();
          const body = element.querySelector<HTMLElement>(".film-roll-body")!;
          const footer = element.querySelector<HTMLElement>(".film-roll-footer")!.getBoundingClientRect();
          return {
            dialog: { left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom },
            footer: { left: footer.left, right: footer.right, bottom: footer.bottom },
            bodyWidth: body.clientWidth,
            bodyScrollWidth: body.scrollWidth,
          };
        });
        expect(bounds.dialog.left, `${width}px: dialog left edge`).toBeGreaterThanOrEqual(0);
        expect(bounds.dialog.right, `${width}px: dialog right edge`).toBeLessThanOrEqual(width);
        expect(bounds.dialog.top, `${width}px: dialog top edge`).toBeGreaterThanOrEqual(0);
        expect(bounds.dialog.bottom, `${width}px: dialog bottom edge`).toBeLessThanOrEqual(820);
        expect(bounds.footer.right, `${width}px: footer stays in dialog`).toBeLessThanOrEqual(bounds.dialog.right);
        expect(bounds.footer.bottom, `${width}px: footer stays in dialog`).toBeLessThanOrEqual(bounds.dialog.bottom);
        expect(bounds.bodyScrollWidth, `${width}px: dialog body has no horizontal overflow`).toBeLessThanOrEqual(bounds.bodyWidth);
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${width}px: page has no horizontal overflow`).toBe(true);

        for (const select of await dialog.locator(".film-roll-field select").all()) {
          const control = await select.evaluate((element) => {
            const bounds = element.getBoundingClientRect();
            const label = element.parentElement!.getBoundingClientRect();
            const style = getComputedStyle(element);
            return { left: bounds.left, right: bounds.right, labelLeft: label.left, labelRight: label.right, paddingRight: parseFloat(style.paddingRight), indicator: style.backgroundImage };
          });
          expect(control.left, `${width}px: select stays within its label`).toBeGreaterThanOrEqual(control.labelLeft);
          expect(control.right, `${width}px: select stays within its label`).toBeLessThanOrEqual(control.labelRight);
          expect(control.paddingRight, `${width}px: select reserves room for its indicator`).toBeGreaterThanOrEqual(34);
          expect(control.indicator, `${width}px: select indicator remains visible`).not.toBe("none");
        }

        const selectedLabel = await dialog.locator(".film-roll-field select").first().evaluate((element) => {
          const style = getComputedStyle(element);
          const context = document.createElement("canvas").getContext("2d")!;
          context.font = style.font;
          const textWidth = context.measureText((element as HTMLSelectElement).selectedOptions[0].textContent ?? "").width;
          const available = element.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight) - parseFloat(style.borderLeftWidth) - parseFloat(style.borderRightWidth);
          return { textWidth, available };
        });
        expect(selectedLabel.textWidth, `${width}px: selected scan type is not clipped`).toBeLessThanOrEqual(selectedLabel.available);
      }

      const photo = await readFile("../../crates/hexscope-core/tests/fixtures/photo.png");
      await dialog.getByLabel("Вибрати скани", { exact: true }).setInputFiles([
        { name: "first.png", mimeType: "image/png", buffer: photo },
        { name: "second.png", mimeType: "image/png", buffer: photo },
      ]);
      await expect(dialog.locator(".film-roll-picker").first()).toContainText("Вибрано 2");
      await expect(dialog.locator(".film-roll-picker").nth(1).locator(".film-roll-picker-count")).toBeHidden();

      const advanced = dialog.locator(".film-roll-advanced");
      const summary = advanced.locator(":scope > summary");
      const [summaryBounds, advancedBounds] = await Promise.all([summary.boundingBox(), advanced.boundingBox()]);
      expect(summaryBounds?.height).toBeGreaterThanOrEqual(44);
      expect(summaryBounds?.width).toBeGreaterThanOrEqual((advancedBounds?.width ?? 0) - 1);
      await summary.click();
      await expect(advanced).toHaveAttribute("open", "");

      await page.keyboard.press("Escape");
      await expect(dialog).not.toBeVisible();
      await expect(page.locator("#film-roll-open")).toBeFocused();
      await page.locator("#film-roll-open").click();
      await page.locator("dialog.film-roll").getByRole("button", { name: "Закрити плівковий рулон" }).click();
      await expect(page.locator("#film-roll-open")).toBeFocused();
    });

    test("photo tools follow the theme and expose accessible controls", async ({ page }) => {
      await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
      await page.goto("./");
      await page.locator("#film-roll-open").click();
      const matchesTheme = () => page.locator(".photo-tool").evaluate((tool) => {
        const reference = document.createElement("div");
        reference.style.backgroundColor = "var(--panel)";
        reference.style.color = "var(--text)";
        document.body.append(reference);
        const actual = getComputedStyle(tool), expected = getComputedStyle(reference);
        const matches = actual.backgroundColor === expected.backgroundColor && actual.color === expected.color;
        reference.remove();
        return matches;
      });
      expect(await matchesTheme()).toBe(true);
      await check(page, "film roll");
      await page.getByRole("button", { name: "Close film roll" }).click();
      const photo = await readFile("../../crates/hexscope-core/tests/fixtures/photo.png");
      await page.locator("#picker-empty").setInputFiles(["a.png", "b.png"].map((name) => ({ name, mimeType: "image/png", buffer: photo })));
      await page.getByRole("button", { name: "Photo privacy mosaic", exact: true }).click();
      const mosaic = page.locator(".photo-mosaic");
      await expect(mosaic.locator("[role=status]")).toContainText("2 / 2 files checked");
      expect(await matchesTheme()).toBe(true);
      await check(page, "photo privacy mosaic");
      const mosaicLayout = await mosaic.evaluate((dialog) => {
        const title = dialog.querySelector("h2")!.getBoundingClientRect();
        const close = dialog.querySelector("button")!.getBoundingClientRect();
        return {
          scrollWidth: dialog.scrollWidth,
          clientWidth: dialog.clientWidth,
          titleBottom: title.bottom,
          closeBottom: close.bottom,
          closeRight: close.right,
          dialogRight: dialog.getBoundingClientRect().right,
        };
      });
      expect(mosaicLayout.scrollWidth).toBeLessThanOrEqual(mosaicLayout.clientWidth);
      expect(Math.abs(mosaicLayout.titleBottom - mosaicLayout.closeBottom)).toBeLessThanOrEqual(12);
      expect(mosaicLayout.closeRight).toBeLessThan(mosaicLayout.dialogRight);
    });

    for (const sample of ["photo.jpg", "redacted.pdf", "phishing.eml", "phishing.msg", "plan.doc", "wifi.png", "budget.xlsx", "broken.png"]) {
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

    test("the clean-copy report is announced and its copy action works from the keyboard", async ({ page }) => {
      page.on("download", (download) => void download.cancel());
      await page.goto("./?sample=photo.jpg");
      await expect(page.locator(".verdict-title")).toBeVisible();
      await page.locator(".verdict-cta").click();
      const report = page.locator(".copy-verification");
      await expect(report).toHaveRole("status");
      await expect(report).toHaveAttribute("aria-live", "polite");
      await expect(report.getByRole("heading", { name: "Removed" })).toBeVisible();
      await expect(page.locator(".film-scan-card")).toBeVisible();
      await check(page, "clean-copy verification report");

      const open = page.getByRole("button", { name: "Open the clean copy" });
      await open.focus();
      await expect(open).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(page.locator(".verdict-title")).toHaveText("Nothing personal found in this photo");
    });

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
