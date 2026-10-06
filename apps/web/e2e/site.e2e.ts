// What a person does on the site, in a real browser: open a sample, read
// the answer, save a clean copy, check an email, go offline. Run on a
// computer's screen and a phone's (playwright.config.ts).
import { Buffer } from "node:buffer";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";

// Whatever the page throws, and whatever its Content-Security-Policy blocks,
// fails the test that met it.
let problems: string[] = [];
test.beforeEach(({ page }) => {
  problems = [];
  page.on("pageerror", (e) => problems.push(`error: ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error" && !/net::ERR_INTERNET_DISCONNECTED|Failed to load resource/.test(m.text())) problems.push(`console: ${m.text()}`);
  });
  page.on("load", () =>
    void page
      .evaluate(() => document.addEventListener("securitypolicyviolation", (e) => console.error(`CSP blocked ${e.violatedDirective}: ${e.blockedURI}`)))
      .catch(() => {}),
  );
});
test.afterEach(() => expect(problems).toEqual([]));

/** The landing page, once its script is running: its doors and pickers are in the page before it. */
async function home(page: Page): Promise<void> {
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
}

/** Use the desktop workspace navigation, or its compact phone switcher. */
async function openBytes(page: Page): Promise<void> {
  const desktop = page.locator("#app-nav [data-app-view='bytes']");
  if (await desktop.isVisible()) await desktop.click();
  else await page.locator("#viewswitch button[data-view='bytes']").click();
}

test("starts with the graphite visual direction and landing navigation", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.clear();
    localStorage.setItem("hexscope.language", "en");
  });
  await home(page);

  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  let navigation = page.locator(".landing-nav");
  if (!(await navigation.isVisible())) {
    await page.locator(".landing-menu > summary").click();
    navigation = page.locator(".landing-menu nav");
  }
  await expect(navigation).toBeVisible();
  await expect(navigation.getByRole("link", { name: "Examples" })).toHaveAttribute("href", "#examples");
  await expect(navigation.getByRole("link", { name: "Formats" })).toHaveAttribute("href", "#formats");
  await expect(navigation.getByRole("link", { name: "Guides" })).toHaveAttribute("href", "#guides");
});

test("landing hero has a concise headline, one primary action, and a real photo preview", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
  await home(page);

  const headline = page.getByRole("heading", { level: 1 });
  await expect(headline).toHaveText("See what your file reveals about you");
  expect(await headline.evaluate((element) => parseFloat(getComputedStyle(element).fontSize))).toBeLessThanOrEqual(48);
  await expect(page.locator(".hero-actions > .btn")).toHaveCount(1);
  await expect(page.locator('.hero-actions a[href="#examples"]')).toBeVisible();

  const preview = page.locator(".demo img.demo-image");
  await expect.poll(() => preview.evaluate((image) => (image as HTMLImageElement).naturalWidth)).toBeGreaterThan(300);
});

test("landing connects its examples to the real file analyzer", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
  await home(page);

  const deeper = page.locator(".deeper-section");
  await expect(deeper).toBeVisible();
  const order = await page.evaluate(() => {
    const examples = document.querySelector("#examples")!.getBoundingClientRect();
    const deeper = document.querySelector(".deeper-section")!.getBoundingClientRect();
    const formats = document.querySelector("#formats")!.getBoundingClientRect();
    return { examplesBottom: examples.bottom, deeperTop: deeper.top, deeperBottom: deeper.bottom, formatsTop: formats.top };
  });
  expect(order.deeperTop).toBeGreaterThan(order.examplesBottom);
  expect(order.deeperBottom).toBeLessThan(order.formatsTop);
  await expect(deeper.getByRole("heading", { name: "Go deeper" })).toBeVisible();
  await deeper.scrollIntoViewIfNeeded();

  const preview = page.locator(".deeper-preview img");
  await expect.poll(() => preview.evaluate((image) => (image as HTMLImageElement).naturalWidth)).toBeGreaterThan(300);
  if ((page.viewportSize()?.width ?? 0) <= 560) {
    expect(await preview.evaluate((image) => (image as HTMLImageElement).currentSrc)).toContain("analyzer-preview-mobile.png");
  }

  await page.locator(".deeper-open-sample").click();
  await expect(page.locator("body")).toHaveAttribute("data-state", "ready");
  await expect(page.locator(".verdict-title")).toBeVisible();
  const desktopBytes = page.locator("#app-nav [data-app-view='bytes']");
  if (await desktopBytes.isVisible()) await desktopBytes.click();
  else await page.locator("#viewswitch button[data-view='bytes']").click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  await expect(page.locator("#hex")).toBeVisible();
});

test("the analyzer preview follows the selected site language", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
  await home(page);
  await page.locator(".deeper-section").scrollIntoViewIfNeeded();

  const preview = page.locator(".deeper-preview img");
  await expect.poll(() => preview.evaluate((image) => (image as HTMLImageElement).naturalWidth)).toBeGreaterThan(300);
  const language = page.locator(".topbar .language-button");
  await language.click();
  await expect.poll(() => preview.evaluate((image) => (image as HTMLImageElement).currentSrc)).toContain("analyzer-preview-uk");

  await language.click();
  await expect.poll(() => preview.evaluate((image) => (image as HTMLImageElement).currentSrc)).not.toContain("analyzer-preview-uk");
});

test("analyzer navigation switches between its existing workspace views", async ({ page }) => {
  test.skip((page.viewportSize()?.width ?? 0) <= 900, "the desktop navigation is replaced by the compact phone switcher");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();

  const nav = page.getByRole("navigation", { name: "Робоча область файла" });
  await expect(nav).toBeVisible();
  for (const label of ["Огляд", "Метадані", "Вміст", "Структура", "Байти", "Стиснення", "Порівняння"]) {
    await expect(nav.getByRole("button", { name: label, exact: true })).toBeVisible();
  }

  await expect(page.locator("body")).toHaveAttribute("data-app-view", "overview");
  await expect(page.locator("#tree")).toBeHidden();
  await expect(page.locator("#hex")).toBeHidden();

  await nav.getByRole("button", { name: "Метадані", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "metadata");
  await expect(page.locator("#drawer .reveals")).toBeInViewport();

  await nav.getByRole("button", { name: "Вміст", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "content");
  await expect(page.locator("#drawer .reveal-hero")).toBeInViewport();

  await nav.getByRole("button", { name: "Структура", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "structure");
  await expect(page.locator("#tree")).toBeVisible();
  await expect(page.locator("#hex")).toBeHidden();

  await nav.getByRole("button", { name: "Байти", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  await expect(page.locator("#tree")).toBeVisible();
  await expect(page.locator("#hex")).toBeVisible();
  const byteCanvas = page.locator("canvas.hex-canvas");
  await expect.poll(() => byteCanvas.evaluate((canvas) => {
    const element = canvas as HTMLCanvasElement;
    if (!element.width || !element.height) return 0;
    const context = element.getContext("2d");
    if (!context) return 0;
    const pixels = context.getImageData(0, 0, element.width, element.height).data;
    let different = 0;
    for (let i = 0; i < pixels.length; i += 4) {
      if (pixels[i] > 45 || pixels[i + 1] > 45 || pixels[i + 2] > 45) different++;
    }
    return different;
  })).toBeGreaterThan(100);

  await nav.getByRole("button", { name: "Огляд", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "overview");
  await expect(page.locator("#tree")).toBeHidden();
  await expect(page.locator("#hex")).toBeHidden();

  await expect(nav.getByRole("button", { name: "Стиснення", exact: true })).toBeDisabled();
  const fileChooserEvent = page.waitForEvent("filechooser");
  await nav.getByRole("button", { name: "Порівняння", exact: true }).click();
  const fileChooser = await fileChooserEvent;
  await fileChooser.setFiles(sample("report.docx"));
  await expect(page.locator("dialog.compare")).toBeVisible();
});

test("photo metadata is grouped and its location still opens the exact bytes", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();

  const metadataNav = page.locator("#app-nav [data-app-view='metadata']");
  if (await metadataNav.isVisible()) await metadataNav.click();

  const metadata = page.locator("#drawer .reveals");
  await expect(metadata.locator(".metadata-group")).toHaveCount(6);
  await expect(metadata.locator(".metadata-group[data-metadata-group='general']")).toContainText("Основна інформація");
  await expect(metadata.locator(".metadata-group[data-metadata-group='camera']")).toContainText("Камера й пристрій");
  await expect(metadata.locator(".metadata-group[data-metadata-group='location']")).toContainText("Місце зйомки");
  await expect(metadata.locator(".metadata-group[data-metadata-group='dates']")).toContainText("Дати");
  await expect(metadata.locator(".metadata-group[data-metadata-group='creator']")).toContainText("Автор і програма");

  const additional = metadata.locator(".metadata-group[data-metadata-group='additional']");
  await expect(additional).toContainText("Додаткові поля");
  await expect(additional.locator("details")).not.toHaveAttribute("open", "");
  await expect(additional.locator("dd[data-kind='thumbnail']")).toBeHidden();
  await additional.locator("summary").click();
  await expect(additional.locator("dd[data-kind='thumbnail']")).toBeVisible();

  const locationRow = metadata.locator(".metadata-group[data-metadata-group='location'] dd[data-kind='location']");
  const locationNode = await locationRow.getAttribute("data-node");
  expect(locationNode).toMatch(/^\d+$/);
  await locationRow.locator(".reveal-link").click();
  const selectedTreeRow = page.locator(".tree .row.is-selected");
  await expect(selectedTreeRow).toHaveAttribute("data-id", locationNode!);
  await expect(selectedTreeRow.locator(".label")).toContainText("GPS IFD");
  const bytesNav = page.locator("#app-nav [data-app-view='bytes']");
  if (await bytesNav.isVisible()) await bytesNav.click();
  else await page.locator("#viewswitch button[data-view='bytes']").click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  await expect(page.locator("#hex")).toBeVisible();
});

test("phone analyzer keeps the compact summary and bytes switch", async ({ page }) => {
  test.skip((page.viewportSize()?.width ?? 1000) > 900, "the desktop uses the full workspace navigation");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await expect(page.locator("#app-nav")).toBeHidden();

  const switcher = page.locator("#viewswitch");
  await expect(switcher).toBeVisible();
  await switcher.getByRole("button", { name: "Байти", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  await expect(page.locator("#hex")).toBeVisible();
  await expect(page.locator("html")).toHaveJSProperty("scrollWidth", page.viewportSize()!.width);
  await switcher.getByRole("button", { name: "Зведення", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "overview");
});

test("desktop analyzer compression navigation toggles the existing player", async ({ page }) => {
  test.skip((page.viewportSize()?.width ?? 0) <= 900, "the phone keeps its existing compact switcher");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=sample.png");

  const compression = page.locator("#app-nav [data-app-action='compression']");
  await expect(compression).toBeEnabled();
  await expect(page.locator("body")).toHaveClass(/is-playing/);
  await expect(page.locator(".drawer-player")).toBeVisible();
  await expect(compression).toHaveAttribute("aria-pressed", "true");

  await compression.click();
  await expect(page.locator("body")).not.toHaveClass(/is-playing/);
  await expect(page.locator(".drawer-player")).toBeHidden();
  await expect(compression).toBeEnabled();
  await expect(compression).toHaveAttribute("aria-pressed", "false");

  await compression.click();
  await expect(page.locator(".drawer-player")).toBeVisible();
  await compression.click();
  await expect(page.locator(".drawer-player")).toBeHidden();
});

test("pairs each landing promise with a consistent outline icon", async ({ page }) => {
  await home(page);

  const icons = page.locator('.landing-proof .promise > svg[aria-hidden="true"]');
  await expect(icons).toHaveCount(3);
  const firstBox = await icons.first().boundingBox();
  expect(firstBox).not.toBeNull();

  for (const icon of await icons.all()) {
    await expect(icon).toHaveCSS("fill", "none");
    await expect(icon).toHaveCSS("stroke-linecap", "round");
    await expect(icon).toHaveCSS("stroke-linejoin", "round");
    expect(await icon.evaluate((element) => getComputedStyle(element).stroke)).not.toBe("none");
    const box = await icon.boundingBox();
    expect(box?.width).toBeGreaterThanOrEqual(18);
    expect(box?.height).toBeGreaterThanOrEqual(18);
    expect(box?.width).toBe(firstBox!.width);
    expect(box?.height).toBe(firstBox!.height);
  }
});

test("keeps the mobile landing menu compact and aligned below the header", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  await home(page);

  const summary = page.locator(".landing-menu > summary");
  const button = await summary.boundingBox();
  expect(button).not.toBeNull();
  expect(button!.height).toBeLessThanOrEqual(36);
  const primaryAction = await page.locator(".topbar .actions > .btn[data-opens='picker']").boundingBox();
  expect(primaryAction).not.toBeNull();
  expect(primaryAction!.x + primaryAction!.width).toBeLessThanOrEqual(390);

  await summary.click();
  const menu = page.locator(".landing-menu nav");
  await expect(menu).toBeVisible();
  const [menuBox, headerBox] = await Promise.all([
    menu.boundingBox(),
    page.locator(".topbar").boundingBox(),
  ]);
  expect(menuBox).not.toBeNull();
  expect(headerBox).not.toBeNull();
  expect(menuBox!.x).toBeGreaterThanOrEqual(button!.x);
  expect(menuBox!.y).toBeGreaterThanOrEqual(headerBox!.y + headerBox!.height + 4);
  expect(menuBox!.x + menuBox!.width).toBeLessThanOrEqual(390);
});

/** Opens a sample from its door on the landing page. */
async function openDoor(page: Page, door: RegExp): Promise<void> {
  await home(page);
  let target = page.getByRole("button", { name: door });
  if ((await target.count()) === 0) {
    await page.locator(".geek-more > summary").click();
    target = page.getByRole("button", { name: door });
  }
  await target.click();
  await expect(page.locator(".verdict-title")).toBeVisible();
}

/** Downloads are written nowhere: each is noted and cancelled. */
function catchDownloads(page: Page): string[] {
  const names: string[] = [];
  page.on("download", (d) => {
    names.push(d.suggestedFilename());
    void d.cancel();
  });
  return names;
}

test("shows local film-border patterns as clues, not proof of a film original", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
  await home(page);
  const bytes = await page.evaluate(async () => {
    const canvas = document.createElement("canvas");
    canvas.width = 320;
    canvas.height = 200;
    const context = canvas.getContext("2d")!;
    context.fillStyle = "rgb(70,70,70)";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.fillStyle = "rgb(245,245,245)";
    for (let x = 16; x < canvas.width - 16; x += 28) {
      context.fillRect(x, 4, 10, 10);
      context.fillRect(x, canvas.height - 14, 10, 10);
    }
    context.fillStyle = "rgb(180,180,180)";
    context.fillRect(22, 22, canvas.width - 44, 3);
    context.fillRect(22, canvas.height - 25, canvas.width - 44, 3);
    context.fillRect(22, 22, 3, canvas.height - 44);
    context.fillRect(canvas.width - 25, 22, 3, canvas.height - 44);
    const blob = await new Promise<Blob>((resolve, reject) =>
      canvas.toBlob((value) => value ? resolve(value) : reject(new Error("could not encode fixture")), "image/jpeg", 1),
    );
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  });

  await page.locator("#picker-empty").setInputFiles({
    name: "film-border.jpg",
    mimeType: "image/jpeg",
    buffer: Buffer.from(bytes),
  });

  const card = page.locator(".film-scan-card");
  await expect(card).toHaveAttribute("data-state", "complete");
  await expect(card.locator(".film-scan-evidence").first()).toContainText("Repeated high-contrast openings");
  await expect(card).toContainText("do not prove that the picture came from film");

  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await expect(card).toContainText("Ознаки сканування плівкового фото");
  await expect(card.locator(".film-scan-evidence").first()).toContainText("Повторювані контрастні отвори");
});

test("the landing page holds still while its demonstration plays", async ({ page }) => {
  await page.goto("./");
  await expect(page.locator(".demo")).toBeVisible();
  const below = page.locator(".doors-title").first();
  const tops: number[] = [];
  for (let i = 0; i < 16; i++) {
    tops.push((await below.boundingBox())!.y);
    await page.waitForTimeout(500);
  }
  // A few pixels once, while the first scene settles; never the jumps of a card resizing.
  expect(Math.max(...tops) - Math.min(...tops)).toBeLessThanOrEqual(12);
  await expect(page.locator(".demo-facts.is-clean")).toBeVisible();
  await page.waitForTimeout(4500);
  await expect(page.locator(".demo-kicker")).toHaveText("Before you send a photo");
  await expect(page.locator(".demo-bytes")).toHaveCount(0);
});

test("the phone demo keeps its copy readable in both languages", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce", colorScheme: "light" });
  await home(page);
  await expect(page.locator(".demo-caption")).toBeVisible();
  await expect(page.locator(".demo-kicker")).toHaveText("Before you send a photo");
  await expect(page.locator(".demo-fact.is-shown")).toHaveCount(4);
  await expect(page.locator(".demo-bytes")).toBeHidden();
  await expect(page.locator(".demo-caption")).toHaveText(/Same photo\./);
  const photoVisible = await page.locator(".demo-image").evaluate((image) => {
    const photo = image as HTMLImageElement;
    return photo.complete && photo.naturalWidth > 0 && photo.naturalHeight > 0;
  });
  expect(photoVisible).toBe(true);

  const check = async (language: string) => {
    for (const width of [360, 390]) {
      await page.setViewportSize({ width, height: 844 });
      const measurement = await page.evaluate(() => {
        const side = document.querySelector(".demo-side")!;
        const caption = document.querySelector(".demo-caption")!;
        const demo = document.querySelector(".demo")!;
        const frame = document.querySelector(".demo-frame")!;
        const sideBox = side.getBoundingClientRect();
        const captionBox = caption.getBoundingClientRect();
        return {
          frameWidthRatio: frame.getBoundingClientRect().width / demo.getBoundingClientRect().width,
          captionSize: parseFloat(getComputedStyle(caption).fontSize),
          kickerSize: parseFloat(getComputedStyle(document.querySelector(".demo-kicker")!).fontSize),
          left: captionBox.left - sideBox.left,
          right: sideBox.right - captionBox.right,
          overflow: document.documentElement.scrollWidth - innerWidth,
        };
      });
      expect(measurement.frameWidthRatio, `${language} image fills the phone demo at ${width}px`).toBeGreaterThan(0.85);
      expect(measurement.captionSize, `${language} caption at ${width}px`).toBeGreaterThanOrEqual(14);
      expect(measurement.kickerSize, `${language} label at ${width}px`).toBeGreaterThanOrEqual(12);
      expect(measurement.left, `${language} caption left edge at ${width}px`).toBeGreaterThanOrEqual(-1);
      expect(measurement.right, `${language} caption right edge at ${width}px`).toBeGreaterThanOrEqual(-1);
      expect(measurement.overflow, `${language} page overflow at ${width}px`).toBeLessThanOrEqual(0);
    }
  };

  await check("English");
  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await expect(page.locator(".demo-caption")).toBeVisible();
  await check("Ukrainian");
  const caption = page.locator(".demo-caption");
  await caption.evaluate((element) => {
    element.textContent = "Same photo. Camera details removed in your browser.";
  });
  await expect(caption).toHaveText("Те саме фото. Дані камери видалено у вашому браузері.");
  await caption.evaluate((element) => {
    element.textContent = "Same photo. 24 bytes removed in your browser.";
  });
  await expect(caption).toHaveText("Те саме фото. У вашому браузері видалено 24 байти.");
});

test("the short demo scene reserves no disproportionate empty band", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await home(page);
  await expect(page.locator(".demo-caption")).toBeVisible();

  for (const width of [360, 390, 645, 768]) {
    await page.setViewportSize({ width, height: 844 });
    await page.waitForTimeout(60);
    const spacing = await page.locator(".demo-side").evaluate((side) => {
      const sideBox = side.getBoundingClientRect();
      const children = [...side.children]
        .filter((child) => getComputedStyle(child).display !== "none" && child.getBoundingClientRect().height > 0)
        .map((child) => child.getBoundingClientRect());
      const contentTop = Math.min(...children.map((box) => box.top));
      const contentBottom = Math.max(...children.map((box) => box.bottom));
      return {
        above: contentTop - sideBox.top,
        below: sideBox.bottom - contentBottom,
        overflow: document.documentElement.scrollWidth - innerWidth,
      };
    });
    expect(spacing.above, `demo content above at ${width}px`).toBeGreaterThanOrEqual(-1);
    expect(spacing.below, `demo content below at ${width}px`).toBeGreaterThanOrEqual(-1);
    expect(spacing.above + spacing.below, `unused demo space at ${width}px`).toBeLessThanOrEqual(80);
    expect(spacing.overflow, `page overflow at ${width}px`).toBeLessThanOrEqual(0);
  }
});

test("the landing page keeps its three everyday examples in one aligned grid", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "the desktop arrangement is checked at desktop widths");
  await home(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  const grid = page.locator(".doors-main");
  await expect(grid).toHaveCount(1);
  const doors = grid.locator(":scope > .door");
  await expect(doors).toHaveCount(3);
  const rows = await doors.evaluateAll((elements) => elements.map((e) => Math.round(e.getBoundingClientRect().top)));
  expect(new Set(rows).size).toBe(1);
});

test("the desktop doors line up their text and actions", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "the card row is checked in its desktop arrangement");
  await home(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  const rows = await page.locator(".doors-main .door").evaluateAll((doors) =>
    [".door-title", ".door-text", ".door-go"].map((selector) =>
      doors.map((door) => Math.round(door.querySelector(selector)!.getBoundingClientRect().top)),
    ),
  );
  // System font metrics can round the same baseline a pixel apart across platforms.
  for (const row of rows) expect(Math.max(...row) - Math.min(...row)).toBeLessThanOrEqual(2);
});

test("the landing page uses the available desktop canvas", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "wide-canvas composition is desktop-only");
  await home(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  const width = await page.locator(".empty-card").evaluate((e) => e.getBoundingClientRect().width);
  expect(width).toBeGreaterThanOrEqual(1080);
});

test("the desktop landing keeps the example beside the words", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "desktop demo composition is checked at desktop widths");
  await home(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  await expect(page.locator(".demo-frame")).toBeVisible();
  const composition = await page.evaluate(() => ({
    frame: document.querySelector(".demo-frame")!.getBoundingClientRect().toJSON(),
    side: document.querySelector(".demo-side")!.getBoundingClientRect().toJSON(),
    heroHeight: document.querySelector(".hero")!.getBoundingClientRect().height,
  }));
  expect(
    Math.abs(
      composition.frame.y + composition.frame.height / 2 - (composition.side.y + composition.side.height / 2),
    ),
  ).toBeLessThanOrEqual(1);
  expect(composition.frame.x).toBeLessThan(composition.side.x);
  expect(composition.heroHeight).toBeLessThanOrEqual(380);
});

test("the desktop demo gives photo facts room to wrap as words", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "desktop fact wrapping is checked across desktop widths");
  await home(page);
  await expect(page.locator(".demo-fact.is-shown")).toHaveCount(4);

  for (const width of [901, 1024, 1280, 1440]) {
    await page.setViewportSize({ width, height: 900 });
    const layout = await page.locator(".demo-side").evaluate((side) => {
      const labelLines = [...side.querySelectorAll(".demo-fact dt")].map((label) => {
        const range = document.createRange();
        range.selectNodeContents(label);
        return range.getClientRects().length;
      });
      const valueLines = [...side.querySelectorAll(".demo-fact dd")].map((value) => {
        const range = document.createRange();
        range.selectNodeContents(value);
        return range.getClientRects().length;
      });
      return { width: side.getBoundingClientRect().width, labelLines, valueLines };
    });
    expect(layout.width, `photo facts column at ${width}px`).toBeGreaterThanOrEqual(220);
    expect(Math.max(...layout.labelLines), `longest photo label at ${width}px`).toBeLessThanOrEqual(1);
    expect(Math.max(...layout.valueLines), `longest photo value at ${width}px`).toBeLessThanOrEqual(2);
  }

  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  for (const width of [901, 1024, 1280, 1440]) {
    await page.setViewportSize({ width, height: 900 });
    const layout = await page.locator(".demo-side").evaluate((side) => {
      const lines = (selector: string) => [...side.querySelectorAll(selector)].map((item) => {
        const range = document.createRange();
        range.selectNodeContents(item);
        return range.getClientRects().length;
      });
      return { labelLines: lines(".demo-fact dt"), valueLines: lines(".demo-fact dd") };
    });
    expect(Math.max(...layout.labelLines), `Ukrainian photo label at ${width}px`).toBeLessThanOrEqual(2);
    expect(Math.max(...layout.valueLines), `Ukrainian photo value at ${width}px`).toBeLessThanOrEqual(2);
  }
});

test("the tablet landing centers short demo scenes in their reserved height", async ({ page }) => {
  await page.setViewportSize({ width: 645, height: 800 });
  await home(page);
  await expect(page.locator(".demo-side")).toBeVisible();
  const spacing = await page.locator(".demo-side").evaluate((side) => {
    const sideBox = side.getBoundingClientRect();
    const visible = [...side.children].filter((child) => getComputedStyle(child).display !== "none");
    const contentTop = Math.min(...visible.map((child) => child.getBoundingClientRect().top));
    const contentBottom = Math.max(...visible.map((child) => child.getBoundingClientRect().bottom));
    return {
      above: contentTop - sideBox.top,
      below: sideBox.bottom - contentBottom,
      overflow: document.documentElement.scrollWidth - innerWidth,
    };
  });
  expect(Math.abs(spacing.above - spacing.below)).toBeLessThanOrEqual(24);
  expect(spacing.overflow).toBeLessThanOrEqual(0);
});

test("the landing page keeps its type readable on a phone", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await home(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator(".demo-fact").first()).toBeAttached();
  const light = await page.evaluate(() => ({
    page: getComputedStyle(document.body).backgroundColor,
    demo: getComputedStyle(document.querySelector(".demo")!).backgroundColor,
  }));
  expect(light).toEqual({ page: "rgb(9, 11, 13)", demo: "rgb(14, 17, 20)" });
  await page.evaluate(() => (document.documentElement.dataset.theme = "dark"));
  const dark = await page.evaluate(() => ({
    page: getComputedStyle(document.body).backgroundColor,
    demo: getComputedStyle(document.querySelector(".demo")!).backgroundColor,
  }));
  expect(dark).toEqual({ page: "rgb(9, 11, 13)", demo: "rgb(14, 17, 20)" });
  const type = await page.evaluate(() => ({
    title: parseFloat(getComputedStyle(document.querySelector(".hero h1")!).fontSize),
    introduction: parseFloat(getComputedStyle(document.querySelector(".hero .lede")!).fontSize),
    example: parseFloat(getComputedStyle(document.querySelector(".doors-main .door-text")!).fontSize),
    demoLabel: parseFloat(getComputedStyle(document.querySelector(".demo-kicker")!).fontSize),
    demoCaption: parseFloat(getComputedStyle(document.querySelector(".demo-caption")!).fontSize),
    demoFact: parseFloat(getComputedStyle(document.querySelector(".demo-fact")!).fontSize),
    demoFactValue: parseFloat(getComputedStyle(document.querySelector(".demo-fact dd")!).fontSize),
    primaryActionHeight: Math.min(
      ...[...document.querySelectorAll<HTMLButtonElement>(".hero-actions .btn-big")].map((button) =>
        button.getBoundingClientRect().height,
      ),
    ),
    bodyFont: getComputedStyle(document.body).fontFamily,
    codeFont: getComputedStyle(document.querySelector(".tree")!).fontFamily,
  }));
  expect(type.title).toBeGreaterThanOrEqual(30);
  expect(type.introduction).toBeGreaterThanOrEqual(16);
  expect(type.example).toBeGreaterThanOrEqual(14);
  expect(type.demoLabel).toBeGreaterThanOrEqual(12);
  expect.soft(type.demoCaption).toBeGreaterThanOrEqual(14);
  expect.soft(type.demoFact).toBeGreaterThanOrEqual(14);
  expect.soft(type.demoFactValue).toBeGreaterThanOrEqual(14);
  expect(type.primaryActionHeight).toBeGreaterThanOrEqual(44);
  expect(type.bodyFont).toContain("system-ui");
  expect(type.codeFont).toContain("monospace");
});

test("the tablet demo keeps photo facts at reading size", async ({ page }) => {
  await home(page);
  await page.setViewportSize({ width: 768, height: 900 });
  await expect(page.locator(".demo-fact").first()).toBeAttached();
  const type = await page.evaluate(() => ({
    label: parseFloat(getComputedStyle(document.querySelector(".demo-fact dt")!).fontSize),
    value: parseFloat(getComputedStyle(document.querySelector(".demo-fact dd")!).fontSize),
    overflow: document.documentElement.scrollWidth - innerWidth,
  }));
  expect(type.label).toBeGreaterThanOrEqual(14);
  expect(type.value).toBeGreaterThanOrEqual(14);
  expect(type.overflow).toBeLessThanOrEqual(0);
});

test("shared routes only request their current stylesheets", async ({ page }) => {
  const stylesheetUrls = () =>
    page.evaluate(() =>
      performance
        .getEntriesByType("resource")
        .map((entry) => entry.name)
        .filter((url) => new URL(url).pathname.endsWith(".css")),
    );

  await home(page);
  expect(await stylesheetUrls()).toHaveLength(1);
  await page.goto("./check-document-before-sending.html");
  await page.waitForLoadState("networkidle");
  expect(await stylesheetUrls()).toHaveLength(3);
});

test("supported file types stay grouped in the mobile catalog", async ({ page }) => {
  await home(page);
  await page.setViewportSize({ width: 390, height: 844 });
  const formats = page.locator(".format-catalog");
  await expect(formats).toBeVisible();
  await expect(formats.getByRole("heading", { name: "Supported formats" })).toBeVisible();
  await expect(formats.locator(".format-category")).toHaveCount(4);
  expect(await formats.locator(".format-catalog-grid").evaluate((grid) => getComputedStyle(grid).gridTemplateColumns.trim().split(/\s+/).length)).toBe(1);
});

test("the phone brings the main actions before supporting detail", async ({ page }) => {
  await home(page);
  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  for (const width of [320, 360, 390, 430]) {
    await page.setViewportSize({ width, height: 844 });
    const actions = await page.locator(".hero-actions").boundingBox();
    expect(actions, `missing main actions at ${width}px`).not.toBeNull();
    expect(actions!.y + actions!.height, `main actions leave the first screen at ${width}px`).toBeLessThan(844);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
  }
  const doors = await page.locator(".doors-main").boundingBox();
  const proof = await page.locator(".landing-proof").boundingBox();
  const actions = await page.locator(".hero-actions").boundingBox();
  expect(doors).not.toBeNull();
  expect(proof).not.toBeNull();
  expect(actions).not.toBeNull();
  expect(proof!.y).toBeGreaterThan(actions!.y + actions!.height);
  expect(proof!.y + proof!.height).toBeLessThan(doors!.y);
});

test("landing cards stay inside the content grid across widths and locales", async ({ page }) => {
  await home(page);
  const tabletLayouts: { width: number; locale: "en" | "uk"; lastCardLayout: string }[] = [];
  for (const locale of ["en", "uk"] as const) {
    await page.evaluate((language) => localStorage.setItem("hexscope.language", language), locale);
    await page.reload();
    await page.waitForFunction(() => document.body.dataset.state === "empty");
    await expect(page.locator("html")).toHaveAttribute("lang", locale);

    for (const width of [768, 901, 1024, 1034, 1180, 1280, 1440, 1920]) {
      await page.setViewportSize({ width, height: 900 });
      const layout = await page.locator(".doors-main").evaluate((grid) => {
        const frame = grid.getBoundingClientRect();
        const cards = [...grid.querySelectorAll(":scope > .door")].map((card) => {
          const bounds = card.getBoundingClientRect();
          return {
            left: bounds.left,
            right: bounds.right,
            width: bounds.width,
            layout: getComputedStyle(card).gridTemplateColumns,
          };
        });
        return {
          frame: { left: frame.left, right: frame.right, width: frame.width },
          cards,
          overflow: document.documentElement.scrollWidth - innerWidth,
          viewport: innerWidth,
        };
      });
      expect(layout.cards, `${locale} at ${width}px: all three everyday doors`).toHaveLength(3);
      expect(layout.overflow, `${locale} at ${width}px: page overflow`).toBeLessThanOrEqual(0);
      for (const [index, card] of layout.cards.entries()) {
        expect(card.left, `${locale} at ${width}px: card ${index + 1} left edge`).toBeGreaterThanOrEqual(layout.frame.left - 1);
        expect(card.right, `${locale} at ${width}px: card ${index + 1} right edge inside grid`).toBeLessThanOrEqual(layout.frame.right + 1);
        expect(card.left, `${locale} at ${width}px: card ${index + 1} left edge inside viewport`).toBeGreaterThanOrEqual(-1);
        expect(card.right, `${locale} at ${width}px: card ${index + 1} right edge inside viewport`).toBeLessThanOrEqual(layout.viewport + 1);
        expect(card.width, `${locale} at ${width}px: card ${index + 1} stays readable`).toBeGreaterThanOrEqual(280);
      }
      if (width >= 901 && width <= 1180) {
        tabletLayouts.push({ width, locale, lastCardLayout: layout.cards[2].layout });
      }
    }
  }
  for (const tablet of tabletLayouts) {
    expect(tablet.lastCardLayout.trim().split(/\s+/), `${tablet.locale} at ${tablet.width}px: third door uses one card column`).toHaveLength(1);
  }
});

test("specialist tools group film and multi-photo entry points away from everyday doors", async ({ page }) => {
  await home(page);
  const tools = page.locator(".specialist-tools");
  await expect(tools).toBeVisible();
  await expect(tools.getByRole("heading", { name: "Specialist tools" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Film roll · JPEG / TIFF" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Try a synthetic film negative →" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Choose multiple photos for a privacy mosaic →" })).toHaveAttribute("data-opens", "picker-empty");
  await expect(page.locator(".doors-main #film-roll-open")).toHaveCount(0);
  await expect(page.locator(".doors-main [data-name='film-negative.png']")).toHaveCount(0);

  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await expect(tools.getByRole("heading", { name: "Спеціальні інструменти" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Плівковий рулон · JPEG / TIFF" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Спробувати синтетичний плівковий негатив →" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Вибрати кілька фото для мозаїки приватності →" })).toHaveAttribute("data-opens", "picker-empty");

  await page.getByRole("button", { name: "Switch language to English" }).click();
  const fileChooserEvent = page.waitForEvent("filechooser");
  await tools.getByRole("button", { name: "Choose multiple photos for a privacy mosaic →" }).click();
  const fileChooser = await fileChooserEvent;
  expect(fileChooser.isMultiple()).toBe(true);
  const photo = readFileSync("../../crates/hexscope-core/tests/fixtures/photo.png");
  await fileChooser.setFiles([
    { name: "first-photo.png", mimeType: "image/png", buffer: photo },
    { name: "second-photo.png", mimeType: "image/png", buffer: photo },
  ]);
  await expect(page.locator("#batch")).toContainText("Photo privacy mosaic");
  await expect(page.locator("#batch")).toContainText("2 files");
});

test("the landing page fits the viewport across responsive breakpoints", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "one browser sweeps the full width matrix");
  await home(page);
  const widths = [320, 360, 390, 430, 600, 768, 900, 901, 1024, 1280, 1440, 1920];
  const check = async (where: string) => {
    for (const width of widths) {
      await page.setViewportSize({ width, height: 900 });
      const result = await page.evaluate(() => {
        const visible = [...document.querySelectorAll(".pane-tree, .pane-hex, .pane-drawer")].filter(
          (e) => getComputedStyle(e).display !== "none",
        );
        const outside = visible
          .map((e) => ({ name: e.className, left: e.getBoundingClientRect().left, right: e.getBoundingClientRect().right }))
          .filter((r) => r.left < -1 || r.right > innerWidth + 1);
        return { overflow: document.documentElement.scrollWidth - innerWidth, outside };
      });
      expect(result.overflow, `${where}: document overflow at ${width}px`).toBeLessThanOrEqual(0);
      expect(result.outside, `${where}: a pane leaves the viewport at ${width}px`).toEqual([]);
    }
  };

  await check("landing");
  await openDoor(page, /Check a photo/);
  await check("summary");
  await openBytes(page);
  await check("bytes");
});

test("the desktop summary uses a composed wide layout", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "desktop summary is checked at desktop widths");
  await openDoor(page, /Check a photo/);
  await page.setViewportSize({ width: 1440, height: 900 });
  const width = await page.locator(".drawer-file").evaluate((e) => e.getBoundingClientRect().width);
  expect(width).toBeGreaterThanOrEqual(1000);
});

test("the phone bytes view reserves room for reading the bytes", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the compact tree is specific to touch screens");
  await openDoor(page, /Check a photo/);
  await page.locator("#viewswitch button[data-view='bytes']").click();
  const treeHeight = await page.locator(".pane-tree").evaluate((e) => e.getBoundingClientRect().height);
  expect(treeHeight).toBeLessThanOrEqual((await page.evaluate(() => innerHeight)) * 0.2);
});

test("the phone can inspect a photo in Ukrainian and return to its clean-copy action", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the narrow inspection flow is phone-specific");
  await page.setViewportSize({ width: 320, height: 780 });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await page.locator("#viewswitch button[data-view='bytes']").click();
  await page.waitForTimeout(400);
  await expect(page.locator(".tour")).toHaveCount(0);

  await expect(page.locator(".hex-canvas")).toHaveAttribute(
    "aria-label",
    "Байти файла у шістнадцятковому та текстовому вигляді",
  );
  await expect(page.locator("#status")).toHaveText("Торкніться байта");
  expect(await page.locator("#status").evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
  const canvasBox = await page.locator(".hex-canvas").boundingBox();
  if (!canvasBox) throw new Error("The byte canvas is not visible");
  await page.touchscreen.tap(canvasBox.x + 85, canvasBox.y + 24);
  await expect(page.locator(".drawer-node .node-label")).toHaveText("Вибрана частина");
  await expect(page.locator(".drawer-node")).toContainText("Початок зображення: перші два байти кожного JPEG.");
  await expect(page.locator(".drawer-node")).toContainText("Зсув");
  await expect(page.locator(".drawer-node")).toContainText("Довжина");
  await expect(page.locator(".drawer-node")).toContainText("Тип");
  await expect(page.locator(".drawer-node")).toContainText("Копіювати як");
  const copyBottom = await page.locator(".copy-bytes").evaluate((el) => Math.ceil(el.getBoundingClientRect().bottom));
  const drawerBottom = await page.locator(".drawer-node").evaluate((el) => Math.floor(el.getBoundingClientRect().bottom));
  expect(copyBottom).toBeLessThanOrEqual(drawerBottom);
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);

  await page.locator("#viewswitch button[data-view='summary']").click();
  await expect(page.locator(".verdict-cta")).toBeVisible();
});

test("the byte grid lets a keyboard user move to and pin the exact byte", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator("#viewswitch button[data-view='bytes']").click();
  const bytes = page.locator(".hex-scroller");
  await bytes.focus();

  await page.keyboard.press("ArrowRight");
  await expect(page.locator("#status")).toContainText("0x000000ED");
  await page.keyboard.press("Home");
  const homeStatus = await page.locator("#status").textContent();
  const rowStart = Number.parseInt(homeStatus?.match(/0x[\dA-F]+/)?.[0].slice(2) ?? "", 16);
  expect(Number.isFinite(rowStart)).toBe(true);
  expect(rowStart).toBeLessThanOrEqual(0xEC);
  for (let offset = rowStart; offset < 0xEC; offset++) await page.keyboard.press("ArrowRight");
  await expect(page.locator("#status")).toContainText("0x000000EC");
  await page.keyboard.press("Enter");
  await expect(page.locator(".drawer-node")).toContainText("GPS IFD");
});

test("the selected byte explanation is labeled apart from the file summary", async ({ page }) => {
  await openDoor(page, /Check a photo/);
  await openBytes(page);
  await page.locator(".row").first().click();
  await expect(page.locator(".drawer-node .node-label")).toHaveText("Selected part");
});

test("the pinned byte label remains readable", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "the pinned label is checked in the desktop details pane");
  await openDoor(page, /Check a photo/);
  await openBytes(page);
  await expect(page.locator(".drawer-node .pin")).toBeVisible();
  const pinSize = await page.locator(".drawer-node .pin").evaluate((e) => parseFloat(getComputedStyle(e).fontSize));
  expect(pinSize).toBeGreaterThanOrEqual(12);
});

test("the landing page has a clear primary action, three everyday doors, and a remembered Ukrainian choice", async ({ page }) => {
  await home(page);
  await expect(page.getByRole("button", { name: "Choose a file" })).toBeVisible();
  await expect(page.locator('.hero-actions a[href="#examples"]')).toHaveText("See examples");
  await expect(page.locator(".doors-main .door")).toHaveCount(3);
  expect(await page.locator(".geek-more").evaluate((e) => (e as HTMLDetailsElement).open)).toBe(false);
  expect(await page.locator(".landing-more").evaluate((e) => (e as HTMLDetailsElement).open)).toBe(false);

  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "uk");
  await expect(page.locator(".format-catalog h2")).toHaveText("Підтримувані формати");
  await expect(page.locator("h1")).toHaveText("Дізнайтеся, що файл розкриває про вас");
  await page.getByRole("button", { name: "Switch language to English" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.locator("h1")).toHaveText("See what your file reveals about you");
  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "uk");
  await page.reload();
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await expect(page.locator("html")).toHaveAttribute("lang", "uk");
  await expect(page.getByRole("button", { name: "Вибрати файл" })).toBeVisible();
  await page.locator(".doors-main .door").first().click();
  await expect(page.locator(".verdict-title")).toContainText("Фото розкриває");
  await expect(page.locator(".verdict-lines .is-reveals")).toContainText("Розкриває місце зйомки");
  await expect(page.locator("#location")).toHaveAttribute("title", "У фото записано місце зйомки — натисніть, щоб побачити дані");
  await expect(page.locator(".advice")).toContainText("Серійний номер є в кожному фото з цього фотоапарата.");
  await expect(page.locator(".advice")).toContainText("Ім’я власника береться з налаштувань фотоапарата.");
});

test("a guide changes its full article and language metadata with the shared control", async ({ page }) => {
  await page.goto("./is-this-email-real.html");
  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await expect(page.locator("article.story-text")).toHaveAttribute("lang", "uk");
  await expect(page.locator(".guide-language-note")).toHaveCount(0);
  await expect(page.locator("article.story-text h1")).toHaveText("Цей лист справжній?");
  await expect(page).toHaveTitle("Цей лист справжній? — hexscope");
  await expect(page.locator("html")).toHaveAttribute("lang", "uk");
  await expect(page.locator("meta[name=description]")).toHaveAttribute("content", /[А-Яа-яІіЇїЄєҐґ]/);
  await expect(page.locator("meta[property='og:title']")).toHaveAttribute("content", "Цей лист справжній?");
  await expect(page.locator("meta[property='og:description']")).toHaveAttribute("content", /[А-Яа-яІіЇїЄєҐґ]/);
  await expect(page.getByRole("link", { name: "Відкрити файл" })).toBeVisible();
  await page.getByRole("button", { name: "Switch language to English" }).click();
  await expect(page.locator("article.story-text")).toHaveAttribute("lang", "en");
  await expect(page.locator("article.story-text h1")).toHaveText("Is this email real?");
  await expect(page).toHaveTitle("Is this email real? — hexscope");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.locator("meta[name=description]")).toHaveAttribute(
    "content",
    "A message can claim any sender. Its headers say where replies really go, whether the sender's domain vouched for it, and where it came from. How to save an email and read them, in your browser, without uploading it.",
  );
  await expect(page.locator("meta[property='og:title']")).toHaveAttribute("content", "Is this email real?");
  await expect(page.locator("meta[property='og:description']")).toHaveAttribute(
    "content",
    "Where replies really go, whether the sender's domain vouched for the message, and where it came from: read an email's headers without uploading it.",
  );
});

test("nothing scrolls sideways", async ({ page }) => {
  await page.goto("./");
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
  await openDoor(page, /Check a photo/);
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
});

test("the Ukrainian photo toolbar fits a 320px viewport", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 844 });
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  const toolbar = page.locator(".topbar .actions");
  await expect(toolbar).toBeVisible();
  await expect(page.locator(".topbar .location-badge")).toBeVisible();
  await expect(page.locator('.topbar .viewswitch [data-view="bytes"]')).toBeVisible();
  const bounds = await toolbar.evaluate((element) => {
    const rect = element.getBoundingClientRect();
    return { left: rect.left, right: rect.right };
  });
  expect(bounds.left).toBeGreaterThanOrEqual(0);
  expect(bounds.right).toBeLessThanOrEqual(320);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test("the landing page does not download article copy before a guide opens", async ({ page }) => {
  const guideCopyScripts: Promise<string | null>[] = [];
  page.on("response", (response) => {
    if (response.request().resourceType() === "script") {
      guideCopyScripts.push(
        response
          .text()
          .then((body) => (body.includes("On a scanned page it is worse") ? response.url() : null))
          .catch(() => null),
      );
    }
  });
  await home(page);
  await page.waitForLoadState("networkidle");
  expect((await Promise.all(guideCopyScripts)).filter(Boolean)).toEqual([]);
});

test("file results show the summary before evidence and available actions", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  page.on("download", (download) => void download.cancel());
  const examples = [
    { sample: "photo.jpg", evidence: ".reveals", action: ".verdict-actions .verdict-cta" },
    { sample: "report.pdf", evidence: ".reveals", action: ".redactor input[type='search']" },
    { sample: "phishing.eml", evidence: ".reveals", action: ".verdict-do" },
  ];

  for (const example of examples) {
    await page.goto(`./?sample=${example.sample}`);
    await expect(page.locator(".verdict-title")).toBeVisible();
    const result = page.locator(".drawer-file");
    const order = await result.evaluate((root) => {
      const children = [...root.children];
      return {
        verdict: children.findIndex((element) => element.matches(".verdict")),
        evidence: children.findIndex((element) => element.matches(".reveals")),
        redaction: children.findIndex((element) => element.matches(".redactor")),
        details: children.findIndex((element) => element.matches(".more-details")),
      };
    });
    expect(order.verdict, `${example.sample}: summary starts the result`).toBe(0);
    expect(order.evidence, `${example.sample}: evidence follows the summary`).toBeGreaterThan(order.verdict);
    expect(order.details, `${example.sample}: technical details stay last`).toBeGreaterThan(order.evidence);
    if (example.sample === "report.pdf") {
      expect(order.redaction, "PDF redaction follows evidence").toBeGreaterThan(order.evidence);
      await expect(result.locator(example.action)).toBeVisible();
    } else {
      await expect(result.locator(example.action)).toBeVisible();
    }

    const details = result.locator(".more-details");
    await expect(details).not.toHaveAttribute("open", "");
    await expect(details.locator(".makeup")).toBeHidden();
    await details.locator("summary").click();
    await expect(details.locator(".makeup")).toBeVisible();

    if (example.sample === "photo.jpg") {
      await result.locator(example.action).click();
      const comparison = page.locator(".before-after");
      await expect(comparison).toBeVisible();
      await page.setViewportSize({ width: 390, height: 844 });
      const columns = await comparison.evaluate((element) => getComputedStyle(element).gridTemplateColumns.trim().split(/\s+/).length);
      expect(columns, "phone clean-copy comparison stacks the two results").toBe(1);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    }
  }
});

test("a photo: the answer, where it was taken, and an evidence-based clean-copy result", async ({ page }) => {
  const saved = catchDownloads(page);
  await openDoor(page, /Check a photo/);
  await expect(page.locator(".verdict-title")).toHaveText(/^This photo gives away \d+ things$/);
  await expect(page.locator(".more-details > summary")).toHaveText("For the curious");
  await expect(page.locator(".makeup")).toBeHidden();
  await expect(page.locator(".sharer")).toBeHidden();
  await expect(page.locator(".cleaner .btn-clean")).toBeHidden();
  await expect(page.locator(".verdict-actions .verdict-cta")).toBeVisible();
  await expect(page.locator(".tour")).toContainText("fact in the summary");
  // Where the keyboard starts: the answer.
  await expect(page.locator(".verdict-title")).toBeFocused();
  await expect(page.locator(".place-map svg")).toBeVisible();

  await page.locator(".verdict-cta").click();
  const after = page.locator(".ba-side.is-after");
  await expect(after.locator(".ba-count")).toHaveText("4");
  await expect(after.locator(".ba-what")).toHaveText("Confirmed removed");
  await expect(after.locator(".ba-verification-item")).toHaveCount(2);
  await expect(after.locator(".ba-verification-item").nth(0)).toContainText("Still present");
  await expect(after.locator(".ba-verification-item").nth(0).locator("strong")).toHaveText("0");
  await expect(after.locator(".ba-verification-item").nth(1)).toContainText("Not checked");
  await expect(after.locator(".ba-verification-item").nth(1).locator("strong")).toHaveText("4");
  await expect(page.locator(".ba-side.is-after")).not.toHaveClass(/is-clear/);
  await expect(page.locator(".copy-verification")).toContainText("Not checked");
  // A computer saves it; a phone may offer to share it instead.
  if (saved.length > 0) expect(saved).toEqual(["photo-clean.jpg"]);
});

test("clean copy verification reparses a generated photo", async ({ page }) => {
  page.on("download", (download) => void download.cancel());
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator(".verdict-cta").click();

  const report = page.locator(".copy-verification");
  await expect(report).toHaveRole("status");
  await expect(report.getByRole("heading", { name: "Removed" })).toBeVisible();
  const removed = report.locator(".copy-verification-group").nth(0);
  await expect(removed).toContainText("Camera");
  await expect(removed).toContainText("Location");
  await expect(report).toContainText("Still present");
  await expect(report).toContainText("Not checked");
  await expect(report).toContainText("Hexscope cannot yet confirm whether this JPEG finding was removed.");
  await expect(report).not.toContainText("48.8584");
  await expect(page.locator(".before-after .ba-side.is-after")).not.toHaveClass(/is-clear/);
});

test("clean-copy coverage explanations are translated in Ukrainian", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  page.on("download", (download) => void download.cancel());
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator(".verdict-cta").click();

  await expect(page.locator(".copy-verification")).toContainText(
    "Hexscope поки не може підтвердити, чи видалено цей факт у форматі JPEG.",
  );
});

test("clean copy verification checks that selected PDF text left the searchable copy", async ({ page }) => {
  page.on("download", (download) => void download.cancel());
  await page.goto("./?sample=report.pdf");
  await expect(page.locator(".verdict-title")).toBeVisible();
  const redactor = page.locator(".redactor");
  await redactor.locator('input[type="search"]').fill("Salary");
  await redactor.getByRole("button", { name: "Find", exact: true }).click();
  await expect(redactor.locator("mark.redact-hit")).toContainText("Salary");
  await redactor.locator(".redact-form button").click();
  await redactor.locator(".btn-primary").click();

  const report = redactor.locator(".copy-verification");
  const removed = report.locator(".copy-verification-group").nth(0);
  await expect(removed.getByRole("heading", { name: "Removed" })).toBeVisible();
  await expect(removed).toContainText("Selected PDF text");
  await expect(report).not.toContainText("Salary");
});

test("the PDF page picker keeps its dropdown arrow clear of the page name", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 844 });
  await page.goto("./?sample=redacted.pdf");
  await expect(page.locator(".verdict-title")).toBeVisible();
  const redactor = page.locator(".redactor");
  await redactor.locator('input[type="search"]').fill("Olena Koval");
  await redactor.getByRole("button", { name: "Find", exact: true }).click();

  const picker = redactor.locator(".redact-page-pick");
  await expect(picker).toBeVisible();
  const style = await picker.evaluate((element) => {
    const computed = getComputedStyle(element);
    return { appearance: computed.appearance, backgroundImage: computed.backgroundImage, paddingRight: parseFloat(computed.paddingRight) };
  });
  expect(style.appearance).toBe("none");
  expect(style.backgroundImage).toContain("linear-gradient");
  expect(style.paddingRight).toBeGreaterThanOrEqual(36);
});

test("clean copy verification announces a controlled result while copy actions stay available", async ({ page }) => {
  await page.addInitScript(() => {
    const NativeWorker = window.Worker;
    class ControlledWorker extends NativeWorker {
      postMessage(message: any, transfer: Transferable[]): void;
      postMessage(message: any, options?: StructuredSerializeOptions): void;
      postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (message && typeof message === "object" && "type" in message && message.type === "verifyCopy" && "id" in message) {
          const id = message.id;
          setTimeout(() => {
            this.dispatchEvent(
              new MessageEvent("message", {
                data: {
                  id,
                  type: "verification",
                  report: {
                    removed: [],
                    present: [
                      { kind: "qrwifi", label: "Wi-Fi in a QR code", reason: "Kept: the QR code, which is part of the picture. Black it out with “Black out part of the picture” before sending.", value: "Private value", scope: "file" },
                      { kind: "camera", label: "Camera", reason: "This finding is still present in the copy." },
                    ],
                    unchecked: [{ kind: "owner", label: "Owner", reason: "This finding has no stable value to compare." }],
                  },
                },
              }),
            );
          }, 900);
          return;
        }
        if (Array.isArray(transferOrOptions)) super.postMessage(message, transferOrOptions);
        else super.postMessage(message, transferOrOptions);
      }
    }
    window.Worker = ControlledWorker;
  });
  page.on("download", (download) => void download.cancel());
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator(".verdict-cta").click();

  const report = page.locator(".copy-verification");
  await expect(report).toContainText("Checking the copy in this tab");
  const tone = await page.locator(".before-after .ba-side.is-after").evaluate((actual) => {
    const expected = document.createElement("div");
    expected.className = "ba-side is-after";
    document.body.append(expected);
    const colors = [getComputedStyle(actual).backgroundColor, getComputedStyle(expected).backgroundColor];
    expected.remove();
    return colors;
  });
  expect(tone[0]).toBe(tone[1]);
  const open = page.getByRole("button", { name: "Open the clean copy" });
  await expect(open).toBeEnabled();
  await expect(page.getByRole("button", { name: "Compare with the original" })).toBeEnabled();
  await expect(report.getByRole("heading", { name: "Still present" })).toBeVisible();
  await expect(report).toContainText("Kept: the QR code");
  await expect(report).toContainText("This finding is still present in the copy.");
  await expect(report.getByRole("heading", { name: "Not checked" })).toBeVisible();
  await expect(report).toContainText("This finding has no stable value to compare.");
  await expect(report).not.toContainText("Private value");
  await expect(page.locator(".before-after .ba-side.is-after")).not.toHaveClass(/is-clear/);
});

test("clean copy verification keeps copy actions when the worker returns an error", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  await page.addInitScript(() => {
    const NativeWorker = window.Worker;
    class ErrorWorker extends NativeWorker {
      postMessage(message: any, transfer: Transferable[]): void;
      postMessage(message: any, options?: StructuredSerializeOptions): void;
      postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (message && typeof message === "object" && "type" in message && message.type === "verifyCopy" && "id" in message) {
          this.dispatchEvent(new MessageEvent("message", { data: { id: message.id, type: "error", message: "Private parser detail" } }));
          return;
        }
        if (Array.isArray(transferOrOptions)) super.postMessage(message, transferOrOptions);
        else super.postMessage(message, transferOrOptions);
      }
    }
    window.Worker = ErrorWorker;
  });
  page.on("download", (download) => void download.cancel());
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator(".verdict-cta").click();

  const report = page.locator(".copy-verification");
  await expect(report.getByRole("heading", { name: "Не перевірено" })).toBeVisible();
  await expect(report).toContainText("Перевірка недоступна.");
  await expect(report).not.toContainText("Private parser detail");
  await expect(page.getByRole("button", { name: "Відкрити очищену копію" })).toBeEnabled();
  await expect(page.getByRole("button", { name: "Порівняти з оригіналом" })).toBeEnabled();
});

test.describe("copy verification module network failure", () => {
  test.use({ serviceWorkers: "block" });

  test("keeps copy actions available when its lazy module fails to load", async ({ page }) => {
    const failed: string[] = [];
    page.on("requestfailed", (request) => failed.push(request.url()));
    await page.route("**/copyverification-*.js", (route) => route.abort());
    page.on("download", (download) => void download.cancel());
    await page.goto("./?sample=photo.jpg");
    await expect(page.locator(".verdict-title")).toBeVisible();
    await page.locator(".verdict-cta").click();

    const report = page.locator(".copy-verification");
    await expect.poll(() => failed.some((url) => /copyverification-.*\.js/.test(url))).toBe(true);
    await expect(report).toContainText("Not checked");
    await expect(report).toContainText("Check unavailable.");
    await expect(page.getByRole("button", { name: "Open the clean copy" })).toBeEnabled();
    await expect(page.getByRole("button", { name: "Compare with the original" })).toBeEnabled();
  });

  test("shows an unchecked result when the lazy module request stalls", async ({ page }) => {
    let intercepted = false;
    let release!: () => void;
    const requestGate = new Promise<void>((resolve) => (release = resolve));
    await page.route("**/copyverification-*.js", async (route) => {
      intercepted = true;
      await requestGate;
      await route.abort();
    });
    page.on("download", (download) => void download.cancel());
    await page.goto("./?sample=photo.jpg");
    await expect(page.locator(".verdict-title")).toBeVisible();
    await page.locator(".verdict-cta").click();

    const report = page.locator(".copy-verification");
    await expect.poll(() => intercepted).toBe(true);
    try {
      await expect(report).toContainText("Check unavailable.", { timeout: 12_000 });
    } finally {
      release();
    }
    await expect(report).toContainText("Not checked");
    await expect(page.getByRole("button", { name: "Open the clean copy" })).toBeEnabled();
  });
});

test("a fake bank email: why it may not be real, drawn on its way", async ({ page }) => {
  await openDoor(page, /Check an email before you trust it/);
  await expect(page.locator(".verdict-title")).toHaveText("This email may not be from who it says");
  await expect(page.locator(".mail-route .mail-stop").first()).toContainText("security@example-bank.com");
  await expect(page.locator(".mail-off .mail-stop.is-bad")).toHaveCount(3);
});

test("a blacked-out PDF: the names still under its boxes", async ({ page }) => {
  await openDoor(page, /Check a document before you send it/);
  await expect(page.locator(".verdict-title")).toHaveText(/^This PDF gives away \d+ things$/);
  await expect(page.locator(".reveal-list")).toContainText("Olena Koval");
});

test("presentation notes are removed only when asked", async ({ page }) => {
  catchDownloads(page);
  const deck = readFileSync(new URL("../../../crates/hexscope-core/tests/fixtures/keynote.pptx", import.meta.url));
  const upload = async () => {
    await home(page);
    await page.locator("#picker-empty").setInputFiles({
      name: "keynote.pptx",
      mimeType: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
      buffer: deck,
    });
    await expect(page.locator(".verdict-title")).toBeVisible();
  };

  await upload();
  const option = page.getByRole("checkbox", { name: "Also empty the comments and the speaker's notes, and who wrote them" });
  await expect(option).not.toBeChecked();
  await page.locator(".verdict-cta").click();
  const problem = page.locator(".cleaner .problem");
  const result = page.locator(".clean-removed");
  await expect(problem.or(result)).toBeVisible();
  if (await problem.isVisible()) {
    await expect(problem).toBeVisible();
  } else {
    await result.locator("summary").click();
    await expect(result).not.toContainText("the speaker's notes, emptied");
  }

  await upload();
  await page.getByRole("checkbox", { name: "Also empty the comments and the speaker's notes, and who wrote them" }).check();
  await page.locator(".verdict-cta").click();
  const removed = page.locator(".clean-removed");
  await expect(removed).toBeVisible();
  await removed.locator("summary").click();
  await expect(removed).toContainText("ppt/notesSlides/notesSlide1.xml: the speaker's notes, emptied");
});

test("a spreadsheet's clean copy keeps what is part of it, and says so", async ({ page }) => {
  catchDownloads(page);
  await page.goto("./?sample=budget.xlsx");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator(".verdict-cta").click();
  const after = page.locator(".ba-side.is-after");
  await expect(after.locator(".ba-count")).toHaveText("0");
  await expect(after.locator(".ba-what")).toHaveText("Findings remain in the copy");
  await expect(after.locator(".ba-verification-item")).toHaveCount(2);
  await expect(after.locator(".ba-verification-item").nth(0)).toContainText("Still present");
  await expect(after.locator(".ba-verification-item").nth(0).locator("strong")).toHaveText("2");
  await expect(after.locator(".ba-verification-item").nth(1)).toContainText("Not checked");
  await expect(after.locator(".ba-verification-item").nth(1).locator("strong")).toHaveText("4");
  await expect(page.locator(".cleaner")).toContainText("Kept: what is part of a workbook or a deck itself");
});

test("a file opened earlier in the tab opens again from the list", async ({ page }) => {
  await openDoor(page, /Check a photo/);
  await page.locator("#picker").setInputFiles(new URL("../public/samples/phishing.eml", import.meta.url).pathname);
  await expect(page.locator(".verdict-title")).toHaveText("This email may not be from who it says");
  const again = page.locator(".recent-files button", { hasText: "photo.jpg" });
  await again.click();
  await expect(page.locator(".verdict-title")).toHaveText(/^This photo gives away/);
  // It lives in memory only: a new page load forgets it.
  await page.reload();
  await expect(page.locator(".recent-files")).toHaveCount(0);
});

test("a photo copied as a clean picture: pixels only, nothing else", async ({ page, context }, info) => {
  test.skip(info.project.name !== "computer", "the clipboard is granted on the computer's browser");
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openDoor(page, /Check a photo/);
  await page.locator(".other-ways > summary").click();
  await page.getByRole("button", { name: "Copy a clean picture" }).first().click();
  await expect(page.getByRole("button", { name: /Copied/ })).toBeVisible();
  const chunks = await page.evaluate(async () => {
    const [item] = await navigator.clipboard.read();
    const b = new Uint8Array(await (await item.getType("image/png")).arrayBuffer());
    const seen: string[] = [];
    for (let i = 8; i + 8 <= b.length; ) {
      const len = ((b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]) >>> 0;
      seen.push(String.fromCharCode(...b.subarray(i + 4, i + 8)));
      i += 12 + len;
    }
    return [...new Set(seen)];
  });
  // No text, no EXIF, no XMP: the picture and nothing else.
  expect(chunks.filter((c) => !["IHDR", "IDAT", "IEND", "sRGB", "gAMA", "cHRM", "pHYs", "iCCP"].includes(c))).toEqual([]);
});

test("the keyboard list closes every way people try: ×, Escape, a click outside, ? again", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "keys are for a computer");
  await home(page);
  const list = page.locator("dialog.shortcuts");
  const open = async () => {
    await page.keyboard.press("?");
    await expect(list).toBeVisible();
  };
  await open();
  await page.getByRole("button", { name: "Close", exact: true }).first().click();
  await expect(list).toHaveCount(0);
  await open();
  await page.keyboard.press("Escape");
  await expect(list).toHaveCount(0);
  await open();
  await page.mouse.click(5, 5);
  await expect(list).toHaveCount(0);
  await open();
  await page.keyboard.press("?");
  await expect(list).toHaveCount(0);
});

const sample = (name: string) => new URL(`../public/samples/${name}`, import.meta.url).pathname;

test("incomplete PDF content warns in Ukrainian and produces no redacted copy", async ({ page }) => {
  const saved = catchDownloads(page);
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await home(page);
  await page.locator("#picker-empty").setInputFiles(new URL("./fixtures/incomplete-form.pdf", import.meta.url).pathname);

  await expect(page.locator(".verdict-line.is-warning")).toContainText(
    "Не весь вміст сторінок або форм у PDF вдалося перевірити. Пошук може пропустити текст.",
  );
  const redactor = page.locator(".redactor");
  await redactor.locator('input[type="search"]').fill("PUBLIC");
  await redactor.locator(".redact-form button").click();
  const warning = redactor.locator(".redact-incomplete");
  await expect(warning).toBeVisible();
  await expect(warning).toContainText("Не весь вміст сторінок або форм у PDF вдалося перевірити.");

  const save = redactor.locator(".btn-primary");
  await expect(save).toBeEnabled();
  await save.click();
  const failure = redactor.locator(".cleaner .problem");
  await expect(failure).toContainText("Копію не створено: перевірку PDF не завершено.");
  await expect(redactor.getByRole("button", { name: "Відкрити копію" })).toHaveCount(0);
  expect(saved).toEqual([]);
});

test("every public guide has a complete Ukrainian version, including its page title", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  const guides = [
    ["black-out-a-pdf.html", "Як правильно приховати дані в PDF"],
    ["check-document-before-sending.html", "Перевірити документ перед надсиланням"],
    ["deflate.html", "Як працює DEFLATE: наочно"],
    ["hidden-text-in-pdf.html", "Невидимий текст у PDF"],
    ["is-this-email-real.html", "Цей лист справжній?"],
    ["pdf-hidden-versions.html", "Видалили з PDF, але текст залишився"],
    ["png-wont-open.html", "Чому не відкривається PNG?"],
    ["remove-location-from-photo.html", "Як прибрати геолокацію з фото"],
    ["what-a-screenshot-gives-away.html", "Що розкриває знімок екрана"],
    ["your-files-stay-private.html", "Ваші файли залишаються на пристрої"],
  ] as const;
  const technical = new Set([
    "hexscope", "pdf", "deflate", "zip", "jpeg", "heic", "webp", "avif", "exif", "xmp", "png", "idat", "iend",
    "ftp", "ascii", "huffman", "iso", "rfc", "cve", "ecma", "w3c", "wasm", "webassembly", "github", "gmail",
    "outlook", "thunderbird", "apple", "android", "iphone", "mac", "windows", "pixel", "chrome", "google", "airdrop",
    "mp4", "quicktime", "mov", "markup", "snipping",
    "spf", "dkim", "dmarc", "eur", "wi", "fi", "qr", "f12", "csp", "css", "xml", "url", "http", "https",
    "rust", "cookie",
    "jbig", "jbig2", "ccitt", "fax", "gzip", "javascript", "crc",
  ]);

  for (const [path, heading] of guides) {
    await page.goto(`./${path}`);
    const article = page.locator(".story-text");
    await expect(article.locator("h1")).toHaveText(heading);
    if (path === "black-out-a-pdf.html") {
      await expect(article.locator(".guide-note").first()).toContainText("Приклад файла — лист із прихованим текстом");
    }
    if (path === "check-document-before-sending.html") {
      await expect(article.locator(".guide-note").first()).toContainText("Приклад файла — Документ Word");
    }
    await expect(article).toHaveAttribute("lang", "uk");
    await expect(page.locator("html")).toHaveAttribute("lang", "uk");
    await expect(page.locator(".guide-language-note")).toHaveCount(0);
    await expect(page).toHaveTitle(`${heading} — hexscope`);
    await expect(page.locator("meta[name=description]")).toHaveAttribute("content", /[А-Яа-яІіЇїЄєҐґ]/);
    await expect(page.locator("meta[property='og:title']")).toHaveAttribute("content", /[А-Яа-яІіЇїЄєҐґ]/);
    await expect(page.locator("meta[property='og:description']")).toHaveAttribute("content", /[А-Яа-яІіЇїЄєҐґ]/);
    const untranslated = await article.evaluate((root, allowed) => {
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
      const leftovers: string[] = [];
      let node: Node | null;
      while ((node = walker.nextNode())) {
        const text = node.textContent?.trim() ?? "";
        if (!text || node.parentElement?.closest("code, script, style, .brand, [data-language-control]")) continue;
        const prose = text
          .replace(/\b(?:Word|Excel|PowerPoint|GitHub Actions?|EXIF|XMP|ECMA-376|Office Open XML|ISO 32000-1)\b/gi, " ")
          .replace(/\bContent-Security-Policy\b/gi, " ")
          .replace(/\bform xobjects?\b/gi, " ")
          .replace(/\bApple Mail\b|\bFrom(?=,)|\bPixel Markup\b|\bSnipping Tool\b/gi, " ");
        const words = prose.toLowerCase().match(/[a-z]{2,}\d*/g) ?? [];
        if (words.some((word) => !allowed.includes(word))) leftovers.push(text);
      }
      return leftovers;
    }, [...technical]);
    expect(untranslated, `${path} contains untranslated prose`).toEqual([]);
  }
});

test("several files: listed, then clean copies of those that give something away", async ({ page }, info) => {
  const saved = catchDownloads(page);
  await home(page);
  await page.locator("#picker-empty").setInputFiles([sample("photo.jpg"), sample("report.docx"), sample("sample.png")]);
  const list = page.locator("#batch");
  await expect(list).toContainText("photo.jpg");
  await expect(list).toContainText("report.docx");
  await expect(page.locator(".batch-title")).toHaveText("2 of 3 files need a look");
  await expect(page.locator(".batch-row", { hasText: "sample.png" }).locator(".batch-answer")).toHaveText("Nothing personal found");
  const save = page.locator("#batch .btn-clean");
  await expect(save).toBeEnabled();
  await save.click();
  if (info.project.name === "computer") {
    await expect.poll(() => saved).toEqual(["hexscope-clean-copies.zip"]);
    await expect(list).toContainText(/Saved \d+ clean cop/);
  } else {
    // A phone that cannot share files saves the archive too.
    await expect(list).toContainText(/clean cop/);
  }
});

test("a broken picture: what is wrong, and a repaired copy", async ({ page }) => {
  const saved = catchDownloads(page);
  await openDoor(page, /Why won't it open/);
  await expect(page.locator(".verdict-title")).toHaveText("This picture is damaged");
  await page.locator(".btn-repair").click();
  await expect.poll(() => saved).toEqual(["broken-repaired.png"]);
});

test("after one visit, it works offline — a document too", async ({ page, context }) => {
  await page.goto("./");
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready;
    // The install keeps the whole build: wait until the full parser is in.
    for (let i = 0; i < 100; i++) {
      const keys = await caches.keys();
      const cache = keys.find((k) => k.startsWith("hexscope-") && k !== "hexscope-shared");
      if (cache) {
        const urls = (await (await caches.open(cache)).keys()).map((r) => r.url);
        if (urls.filter((u) => u.endsWith(".wasm")).length >= 2 && urls.some((u) => u.endsWith("redacted.pdf"))) return;
      }
      await new Promise((r) => setTimeout(r, 200));
    }
    throw new Error("the app was not kept for offline use");
  });
  await context.setOffline(true);
  await page.reload();
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.getByRole("button", { name: /Check a document before you send it/ }).click();
  await expect(page.locator(".verdict-title")).toHaveText(/^This PDF gives away/);
  await page.goto("./black-out-a-pdf.html");
  await expect(page).toHaveTitle(/black out a PDF/i);
});

/** The sample video with its picture and sound grown to `size` bytes, written where the test keeps its files. */
function longVideo(path: string, size: number): string {
  const src = readFileSync(sample("video.mov"));
  const parts: Buffer[] = [];
  for (let at = 0; at + 8 <= src.length; ) {
    const len = src.readUInt32BE(at);
    if (src.toString("latin1", at + 4, at + 8) === "mdat") {
      const box = Buffer.alloc(8 + size);
      box.writeUInt32BE(8 + size, 0);
      src.copy(box, 4, at + 4, at + len);
      parts.push(box);
    } else parts.push(src.subarray(at, at + len));
    at += len;
  }
  writeFileSync(path, Buffer.concat(parts));
  return path;
}

test("a long video: read without its picture and sound, and cleaned all the same", async ({ page }, info) => {
  const path = longVideo(info.outputPath("long-video.mov"), 80 << 20);
  await home(page);
  await page.locator("#picker-empty").setInputFiles(path);
  await expect(page.locator(".verdict-title")).toHaveText(/^This video gives away \d+ things$/);
  await expect(page.locator("dt", { hasText: "Read" }).locator("+ dd")).toContainText("All but the picture and sound");

  const download = page.waitForEvent("download").catch(() => null);
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-count")).toHaveText("0");
  const d = info.project.name === "computer" ? await download : null;
  if (d) {
    expect(d.suggestedFilename()).toBe("long-video-clean.mov");
    expect(statSync(await d.path()).size).toBe(statSync(path).size);
  }
});

test("the bytes have a bar of their own: nothing sits on them", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "the bytes are a tab of their own on a phone");
  await openDoor(page, /Check a photo/);
  await openBytes(page);
  const bytes = (await page.locator(".hex-scroller").boundingBox())!;
  for (const part of [".hex-legend", ".search-open"]) {
    const box = (await page.locator(part).boundingBox())!;
    expect(box.y + box.height, part).toBeLessThanOrEqual(bytes.y);
  }
  await page.keyboard.press("/");
  await page.keyboard.type("hexscope");
  await expect(page.locator(".search-count")).toHaveText(/^1 of \d+$/);
});

test("an Outlook message is read as an email, and an old Word file is cleaned", async ({ page }) => {
  const saved = catchDownloads(page);
  await page.goto("./?sample=phishing.msg");
  await expect(page.locator(".verdict-title")).toHaveText("This email may not be from who it says");
  await expect(page.locator(".reveal-list")).toContainText("invoice.pdf.html");

  await page.goto("./?sample=plan.doc");
  await expect(page.locator(".verdict-title")).toHaveText("This document gives away 2 things");
  await expect(page.locator(".reveal-list")).toContainText("Olena Koval");
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-count")).toHaveText("0");
  if (saved.length > 0) expect(saved).toEqual(["plan-clean.doc"]);
});

test("a QR code: a Wi-Fi password named, and blacked out in a new picture", async ({ page }) => {
  const saved = catchDownloads(page);
  await page.goto("./?sample=wifi.png");
  await expect(page.locator(".verdict-title")).toHaveText("This picture gives away 1 thing");
  await expect(page.locator(".reveal-list")).toContainText("its password “correct horse battery staple”");
  await page.getByRole("button", { name: "Black out the QR code…" }).click();
  await page.getByRole("button", { name: "Make the copy" }).click();
  await page.getByRole("button", { name: "Open the copy" }).click();
  await expect(page.locator(".verdict-title")).toHaveText("Nothing personal found in this picture");
  if (saved.length > 0) expect(saved).toEqual(["wifi-blacked-out.png"]);
});

test("a QR code in a PDF that goes to a lookalike address", async ({ page }) => {
  await home(page);
  await page.locator("#picker-empty").setInputFiles(new URL("../src/qr/fixtures/invoice.pdf", import.meta.url).pathname);
  await expect(page.locator(".reveal-list")).toContainText("shown as “exаmple-bank.com”");
  await expect(page.locator(".reveal-list")).toContainText("in the picture on page 1");
});

test("a QR code a PDF draws in boxes, and an Outlook message kept as RTF", async ({ page }) => {
  await home(page);
  await page.locator("#picker-empty").setInputFiles(new URL("../src/qr/fixtures/vector-qr.pdf", import.meta.url).pathname);
  await expect(page.locator(".reveal-list")).toContainText("in a code drawn on page 1");
  await page.locator("#picker").setInputFiles(new URL("../../../crates/hexscope-core/tests/fixtures/phishing-rtf.msg", import.meta.url).pathname);
  await expect(page.locator(".verdict-title")).toHaveText("This email may not be from who it says");
  await expect(page.locator(".reveal-list")).toContainText("goes to login.example.info");
});
