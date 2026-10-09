import { expect, test } from "@playwright/test";

let pageErrors: string[] = [];

test.beforeEach(({ page }) => {
  pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error" && !/net::ERR_INTERNET_DISCONNECTED|Failed to load resource/.test(message.text())) {
      pageErrors.push(message.text());
    }
  });
  page.on("load", () =>
    void page
      .evaluate(() => document.addEventListener("securitypolicyviolation", (event) => console.error(`CSP blocked ${event.violatedDirective}: ${event.blockedURI}`)))
      .catch(() => {}),
  );
});

test.afterEach(() => expect(pageErrors).toEqual([]));

test("mobile analyzer overview keeps its header and result surfaces composed across widths", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "the responsive width sweep runs in desktop Chromium");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await expect(page.locator(".place-map-svg")).toBeVisible();
  await expect(page.locator(".reveal-picture")).toBeVisible();
  await expect(page.locator(".verdict-kind").first()).toHaveText("Особисті дані");
  await expect(page.locator(".verdict-show")).toHaveText("Знайти у файлі");

  for (const width of [320, 360, 390, 414, 520, 640, 768, 900, 901, 1024, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    const header = await page.evaluate(() => {
      const bounds = (selector: string) => {
        const rect = document.querySelector<HTMLElement>(selector)!.getBoundingClientRect();
        return { left: rect.left, right: rect.right, centerY: (rect.top + rect.bottom) / 2 };
      };
      const fileinfo = document.querySelector<HTMLElement>(".fileinfo")!;
      const filename = document.querySelector<HTMLElement>(".fileinfo .filename")!;
      return {
        brand: bounds(".brand"),
        filename: bounds(".fileinfo .filename"),
        actions: bounds(".topbar .actions"),
        metadataHidden: getComputedStyle(document.querySelector<HTMLElement>(".fileinfo .meta")!).display === "none",
        filenameHasText: Boolean(filename.textContent?.trim()),
        fileinfoWidth: fileinfo.getBoundingClientRect().width,
        scrollWidth: document.documentElement.scrollWidth,
      };
    });
    expect(header.filenameHasText, `filename remains visible at ${width}px`).toBe(true);
    expect(header.brand.right, `brand does not overlap filename at ${width}px`).toBeLessThanOrEqual(header.filename.left + 1);
    expect(header.filename.right, `filename does not overlap actions at ${width}px`).toBeLessThanOrEqual(header.actions.left + 1);
    expect(Math.abs(header.brand.centerY - header.filename.centerY), `brand and filename share a row at ${width}px`).toBeLessThanOrEqual(8);
    expect(Math.abs(header.filename.centerY - header.actions.centerY), `filename and actions share a row at ${width}px`).toBeLessThanOrEqual(8);
    expect(header.fileinfoWidth, `file info has usable width at ${width}px`).toBeGreaterThanOrEqual(40);
    if (width <= 480) expect(header.metadataHidden, `secondary file metadata is hidden at ${width}px`).toBe(true);
    expect(header.scrollWidth, `header does not create horizontal overflow at ${width}px`).toBeLessThanOrEqual(width);

    const surfaces = await page.evaluate(() => {
      const verdict = document.querySelector<HTMLElement>(".drawer-file .verdict")!;
      const finding = document.querySelector<HTMLElement>(".drawer-file .verdict-line")!;
      const reveals = document.querySelector<HTMLElement>(".drawer-file .reveals")!;
      const title = document.querySelector<HTMLElement>(".verdict-title")!;
      const picture = document.querySelector<HTMLElement>(".reveal-picture-frame")!;
      const canvas = document.querySelector<HTMLElement>(".reveal-picture")!;
      const map = document.querySelector<HTMLElement>(".place-map-svg")!;
      const hero = document.querySelector<HTMLElement>(".reveal-hero")!;
      const show = document.querySelector<HTMLElement>(".verdict-show")!;
      const findings = document.querySelector<HTMLElement>(".verdict-findings");
      const pictureBounds = picture.getBoundingClientRect();
      const canvasBounds = canvas.getBoundingClientRect();
      const heroBounds = hero.getBoundingClientRect();
      const showStyle = getComputedStyle(show);
      return {
        verdict: getComputedStyle(verdict).backgroundColor,
        verdictBorderWidth: Number.parseFloat(getComputedStyle(verdict).borderTopWidth),
        finding: getComputedStyle(finding).backgroundColor,
        findingBorderWidth: Number.parseFloat(getComputedStyle(finding).borderTopWidth),
        reveals: getComputedStyle(reveals).backgroundColor,
        radius: Number.parseFloat(getComputedStyle(finding).borderTopLeftRadius),
        titleFocusOutlineStyle: getComputedStyle(title).outlineStyle,
        titleFocusDecoration: getComputedStyle(title).textDecorationLine,
        titleFocusDecorationStyle: getComputedStyle(title).textDecorationStyle,
        titleFocusMarkerRingStyle: getComputedStyle(title, "::before").outlineStyle,
        pictureShadow: getComputedStyle(picture).boxShadow,
        pictureSectionInset: pictureBounds.left - heroBounds.left,
        pictureInset: canvasBounds.left - pictureBounds.left,
        pictureWidthGap: pictureBounds.width - canvasBounds.width,
        mapShadow: getComputedStyle(map).boxShadow,
        findingsDivider: findings ? Number.parseFloat(getComputedStyle(findings).borderLeftWidth) : 0,
        showBackground: showStyle.backgroundColor,
        showPadding: Number.parseFloat(showStyle.paddingLeft),
        showHeight: show.getBoundingClientRect().height,
      };
    });
    expect(surfaces.verdict, `overview summary blends with its canvas at ${width}px`).toBe("rgba(0, 0, 0, 0)");
    expect(surfaces.verdictBorderWidth, `overview summary has no enclosing frame at ${width}px`).toBe(0);
    expect(surfaces.finding, `finding rows blend with their canvas at ${width}px`).toBe("rgba(0, 0, 0, 0)");
    expect(surfaces.findingBorderWidth, `finding rows do not read as individual cards at ${width}px`).toBe(0);
    expect(surfaces.reveals, `evidence blends with its canvas at ${width}px`).toBe("rgba(0, 0, 0, 0)");
    expect(surfaces.radius, `finding rows stay open instead of pill-shaped at ${width}px`).toBe(0);
    expect(surfaces.titleFocusOutlineStyle, `the title focus cue does not add another frame at ${width}px`).toBe("none");
    expect(surfaces.titleFocusDecoration, `keyboard focus remains visible on the answer at ${width}px`).toBe("underline");
    expect(surfaces.titleFocusDecorationStyle, `the focus cue reads differently from a link at ${width}px`).toBe("dotted");
    expect(surfaces.titleFocusMarkerRingStyle, `the status dot does not look like a focused radio control at ${width}px`).toBe("none");
    expect(surfaces.pictureShadow, `the photo is not boxed in at ${width}px`).toBe("none");
    expect(surfaces.pictureSectionInset, `the photo aligns with its section at ${width}px`).toBeLessThanOrEqual(1);
    expect(surfaces.pictureInset, `the photo starts at the left edge of its column at ${width}px`).toBeLessThanOrEqual(1);
    expect(surfaces.pictureWidthGap, `the photo uses its column width at ${width}px`).toBeLessThanOrEqual(1);
    expect(surfaces.mapShadow, `the map is not boxed in at ${width}px`).toBe("none");
    expect(surfaces.findingsDivider, `desktop findings are not separated by another frame at ${width}px`).toBe(0);
    expect(surfaces.showBackground, `the file action reads as a button at ${width}px`).not.toBe("rgba(0, 0, 0, 0)");
    expect(surfaces.showPadding, `the file action has a visible hit area at ${width}px`).toBeGreaterThanOrEqual(8);
    expect(surfaces.showHeight, `the file action is easy to hit at ${width}px`).toBeGreaterThanOrEqual(30);
  }

  for (const view of ["content", "metadata"] as const) {
    await page.locator(`#app-nav [data-app-view='${view}']`).click();
    for (const width of [320, 390, 640, 768, 900, 1024, 1280]) {
      await page.setViewportSize({ width, height: 900 });
      const content = await page.locator("#drawer").evaluate((root) => ({
        clientWidth: root.clientWidth,
        scrollWidth: root.scrollWidth,
        pageWidth: document.documentElement.scrollWidth,
      }));
      expect(content.scrollWidth - content.clientWidth, `${view} stays within its pane at ${width}px`).toBeLessThanOrEqual(0);
      expect(content.pageWidth, `${view} does not widen the page at ${width}px`).toBeLessThanOrEqual(width);
    }
  }

  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator(".verdict-show").click();
  await expect(page.locator('.reveal-list dt[data-kind="location"]')).toHaveClass(/is-flash/);
});
