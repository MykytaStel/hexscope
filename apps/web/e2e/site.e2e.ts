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

/** Use the shared workspace navigation on desktop and phone. */
async function openBytes(page: Page): Promise<void> {
  await page.locator("#app-nav [data-app-view='bytes']").click();
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

test("the phone landing menu anchors its panel to the trigger", async ({ page }) => {
  await home(page);
  await page.setViewportSize({ width: 390, height: 844 });

  const menu = page.locator(".landing-menu");
  const trigger = menu.locator(":scope > summary");
  await trigger.click();

  const panel = menu.locator("nav");
  await expect(panel).toBeVisible();
  const [triggerBox, panelBox] = await Promise.all([trigger.boundingBox(), panel.boundingBox()]);
  expect(triggerBox).not.toBeNull();
  expect(panelBox).not.toBeNull();
  expect(Math.abs((panelBox!.x + panelBox!.width) - (triggerBox!.x + triggerBox!.width))).toBeLessThanOrEqual(1);
  expect(panelBox!.x).toBeGreaterThanOrEqual(8);
  expect(panelBox!.x + panelBox!.width).toBeLessThanOrEqual(382);
});

test("the phone landing menu closes after choosing a destination", async ({ page }) => {
  await home(page);
  await page.setViewportSize({ width: 390, height: 844 });

  const menu = page.locator(".landing-menu");
  await menu.locator(":scope > summary").click();
  await menu.getByRole("link", { name: "Formats" }).click();
  await expect(menu).not.toHaveAttribute("open", "");
});

test("Film Lab shows distinct archival frames and credits their source", async ({ page }) => {
  await home(page);

  const film = page.locator(".film-showcase");
  await expect(film.getByRole("heading", { name: "Film scans" })).toBeVisible();
  const frames = film.locator(".film-frame img");
  await expect(frames).toHaveCount(4);
  await frames.first().scrollIntoViewIfNeeded();
  await expect.poll(() => frames.evaluateAll((images) => images.every((image) => (image as HTMLImageElement).naturalWidth > 1000))).toBe(true);
  const tileHeights = await frames.evaluateAll((images) => images.map((image) => image.getBoundingClientRect().height));
  expect(Math.min(...tileHeights)).toBeGreaterThan(120);
  expect(Math.max(...tileHeights)).toBeLessThanOrEqual(240);
  expect((await film.boundingBox())!.height).toBeLessThan(1100);
  const sources = await frames.evaluateAll((images) => images.map((image) => (image as HTMLImageElement).currentSrc));
  expect(new Set(sources).size).toBe(4);
  await expect(film.getByRole("link", { name: /NASA Apollo 11 archive/i })).toHaveAttribute("href", /nasa\.gov/);
  await expect(film).toContainText(/Published positive scans/);
  await expect(film.getByRole("button", { name: "Process film scans · JPEG / TIFF" })).toBeVisible();
  await expect(film.getByRole("button", { name: "Open frame AS11-40-5903 in Hexscope →" })).toHaveAttribute(
    "data-sample",
    "samples/apollo11/as11-40-5903.jpg",
  );
});

test("the landing shows the camera sample once and keeps its real metadata in the photo choice", async ({ page }) => {
  await home(page);

  const heroPhoto = page.locator(".demo img.demo-image");
  await expect.poll(() => heroPhoto.evaluate((image) => (image as HTMLImageElement).naturalWidth)).toBeGreaterThan(300);
  const photoChoice = page.locator('#examples [data-example-panel="photo"]');
  await expect(photoChoice.locator("img")).toHaveCount(0);
  await expect(photoChoice.locator(".example-file-name")).toHaveText("photo.jpg");
  await expect(photoChoice.locator(".example-facts")).toContainText("Sample Camera X1");
  await expect(photoChoice.locator(".example-facts")).toContainText("HX-000042");
  await expect(photoChoice.locator(".example-evidence-path")).toContainText("JPEG");
  await expect(photoChoice.locator(".example-evidence-path")).toContainText("APP1");
  await expect(photoChoice.locator(".example-evidence-path")).toContainText("GPS IFD");
});

test("the phone photo example has no dead space between its findings and file path", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await home(page);

  const photo = page.locator('#examples [data-example-panel="photo"]');
  const art = photo.locator(".example-photo-art");
  const facts = photo.locator(".example-facts");
  const path = photo.locator(".example-evidence-path");
  const [artBox, factsBox, pathBox] = await Promise.all([art.boundingBox(), facts.boundingBox(), path.boundingBox()]);
  expect(artBox).not.toBeNull();
  expect(factsBox).not.toBeNull();
  expect(pathBox).not.toBeNull();
  expect(pathBox!.y - (factsBox!.y + factsBox!.height)).toBeLessThanOrEqual(24);
  expect(artBox!.y + artBox!.height - (pathBox!.y + pathBox!.height)).toBeLessThanOrEqual(24);
});

test("the film showcase preserves frame proportions and readable actions in narrow layouts", async ({ page }) => {
  await home(page);
  const actions = page.locator(".film-showcase .specialist-actions");
  const frames = page.locator(".film-showcase .film-frame img");

  for (const width of [671, 390]) {
    await page.setViewportSize({ width, height: 900 });
    const columns = await actions.evaluate((element) => getComputedStyle(element).gridTemplateColumns.trim().split(/\s+/).length);
    expect(columns, `${width}px viewport`).toBe(1);
    const primaryActionHeight = await actions.locator(".btn").evaluate((button) => button.getBoundingClientRect().height);
    expect(primaryActionHeight, `${width}px viewport`).toBeLessThanOrEqual(48);
    const aspectRatios = await frames.evaluateAll((images) => images.map((image) => image.getBoundingClientRect().width / image.getBoundingClientRect().height));
    expect(Math.max(...aspectRatios), `${width}px frame proportions`).toBeLessThanOrEqual(1.55);
  }
});

test("the analyzer preview displays its full-resolution source without enlargement", async ({ page }, info) => {
  await home(page);
  const preview = page.locator(".deeper-preview .deeper-image");
  const minWidth = info.project.name === "phone" ? 300 : 1200;
  await preview.scrollIntoViewIfNeeded();
  await expect.poll(() => preview.evaluate((image) => (image as HTMLImageElement).naturalWidth)).toBeGreaterThan(minWidth);
  await expect(preview).toHaveCSS("transform", "none");
});

test("the three example choices update one preview and open their matching sample", async ({ page }) => {
  await home(page);
  const examples = page.locator("#examples");
  const cases = [
    { id: "example-photo", panel: "photo", finding: "48.8584", sample: "photo.jpg", action: "Try a sample photo →" },
    { id: "example-document", panel: "document", finding: "Black boxes that hide nothing", sample: "redacted.pdf", action: "Try a blacked-out PDF →" },
    { id: "example-email", panel: "email", finding: "See where replies go", sample: "phishing.eml", action: "Try a suspicious email →" },
  ];

  await expect(examples.getByRole("radio")).toHaveCount(3);
  for (const item of cases) {
    const choice = examples.locator(`#${item.id}`);
    await examples.locator(`label[for="${item.id}"]`).click();
    await expect(choice).toBeChecked();
    const panel = examples.locator(`[data-example-panel="${item.panel}"]`);
    await expect(panel).toBeVisible();
    await expect(panel).toContainText(item.finding);
    await panel.getByRole("button", { name: item.action }).click();
    await expect(page.locator(".verdict-title")).toBeVisible();
    await expect(page.locator("#fileinfo")).toContainText(item.sample);
    await page.reload();
    await page.waitForFunction(() => document.body.dataset.state === "empty");
  }
});

test("the landing ends its main story with a local privacy action", async ({ page }) => {
  await home(page);

  const privacy = page.locator(".privacy-cta");
  await privacy.scrollIntoViewIfNeeded();
  await expect(privacy.getByRole("heading", { name: "The file never leaves this tab" })).toBeVisible();
  await expect(privacy.getByRole("button", { name: "Open a file" })).toHaveAttribute("data-opens", "picker-empty");
  const advanced = page.locator(".geek-more");
  expect(await privacy.evaluate((section) => section.compareDocumentPosition(document.querySelector(".geek-more")!) & Node.DOCUMENT_POSITION_FOLLOWING)).toBeTruthy();
  await expect(advanced).toBeAttached();
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
  if ((page.viewportSize()?.width ?? 0) > 560) {
    const magnification = await preview.evaluate((image) => image.getBoundingClientRect().width / image.parentElement!.getBoundingClientRect().width);
    expect(magnification).toBeLessThanOrEqual(1.01);
  }
  if ((page.viewportSize()?.width ?? 0) <= 560) {
    expect(await preview.evaluate((image) => (image as HTMLImageElement).currentSrc)).toContain("analyzer-preview-mobile.png");
  }

  await page.locator(".deeper-open-sample").click();
  await expect(page.locator("body")).toHaveAttribute("data-state", "ready");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator("#app-nav [data-app-view='bytes']").click();
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
  test.skip((page.viewportSize()?.width ?? 0) <= 900, "the desktop navigation has a separate responsive phone treatment");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();

  const nav = page.getByRole("navigation", { name: "Робоча область файла" });
  await expect(nav).toBeVisible();
  await expect(nav.locator("button > svg.app-nav-icon[aria-hidden='true']")).toHaveCount(7);
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
  await expect(page.locator("#tree")).toBeInViewport();
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

test("desktop bytes workspace keeps structure labels readable", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "wide analyzer panes are checked on desktop");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator("#app-nav [data-app-view='bytes']").click();

  for (const width of [901, 1024, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    const panes = await page.evaluate(() => Object.fromEntries(
      [".app-nav", ".pane-tree", ".pane-hex", ".pane-drawer"].map((selector) => [
        selector,
        Math.round(document.querySelector(selector)!.getBoundingClientRect().width),
      ]),
    ));
    expect(panes[".pane-tree"], `structure pane at ${width}px`).toBeGreaterThanOrEqual(260);
    expect(panes[".pane-hex"], `bytes pane at ${width}px`).toBeGreaterThanOrEqual(230);
    expect(panes[".pane-drawer"], `details pane at ${width}px`).toBeGreaterThanOrEqual(260);
    const clippedLabels = await page.locator("#tree .label").evaluateAll((labels) =>
      labels
        .filter((label) => label.scrollWidth > label.clientWidth)
        .map((label) => label.textContent),
    );
    expect(clippedLabels, `structure labels at ${width}px`).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
  }
});

test("tablet structure and bytes keep the selected part in a full-width reading pane", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "tablet analyzer layout is checked on desktop Chromium");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator("#app-nav [data-app-view='bytes']").click();

  for (const width of [901, 1024, 1099]) {
    await page.setViewportSize({ width, height: 768 });
    const panes = await page.evaluate(() => {
      const rect = (selector: string) => {
        const { x, y, width: w, bottom, right } = document.querySelector(selector)!.getBoundingClientRect();
        return { x, y, width: w, bottom, right };
      };
      return {
        navigation: rect(".app-nav"),
        tree: rect(".pane-tree"),
        bytes: rect(".pane-hex"),
        details: rect(".pane-drawer"),
        documentWidth: document.documentElement.scrollWidth,
      };
    });
    expect(panes.navigation.right, `navigation sits to the left of the tree at ${width}px`).toBeLessThanOrEqual(panes.tree.x + 1);
    expect(panes.tree.width, `tree width at ${width}px`).toBeGreaterThanOrEqual(340);
    expect(panes.bytes.width, `bytes width at ${width}px`).toBeGreaterThanOrEqual(360);
    expect(Math.abs(panes.tree.y - panes.bytes.y), `tree and bytes share a row at ${width}px`).toBeLessThanOrEqual(1);
    expect(panes.details.x, `details align with tree at ${width}px`).toBeCloseTo(panes.tree.x, 0);
    expect(panes.details.y, `details sit below the work area at ${width}px`).toBeGreaterThanOrEqual(panes.tree.bottom - 1);
    expect(panes.details.width, `details use the full work area at ${width}px`).toBeGreaterThanOrEqual(700);
    expect(panes.documentWidth - width, `no horizontal overflow at ${width}px`).toBeLessThanOrEqual(0);
  }

  await page.locator("#tree .row").first().click();
  await expect(page.locator(".pane-drawer .drawer-node .node-label")).toHaveText("Вибрана частина");
  await expect(page.locator(".pane-drawer .drawer-node")).toBeVisible();

  await page.locator("#app-nav [data-app-view='structure']").click();
  for (const width of [901, 1024, 1099]) {
    await page.setViewportSize({ width, height: 768 });
    const panes = await page.evaluate(() => {
      const tree = document.querySelector(".pane-tree")!.getBoundingClientRect();
      const details = document.querySelector(".pane-drawer")!.getBoundingClientRect();
      return { tree, details, documentWidth: document.documentElement.scrollWidth };
    });
    expect(panes.tree.width, `structure width at ${width}px`).toBeGreaterThanOrEqual(700);
    expect(panes.details.x, `structure details align at ${width}px`).toBeCloseTo(panes.tree.x, 0);
    expect(panes.details.y, `structure details sit below at ${width}px`).toBeGreaterThanOrEqual(panes.tree.bottom - 1);
    expect(panes.details.width, `structure details use the full work area at ${width}px`).toBeGreaterThanOrEqual(700);
    expect(panes.documentWidth - width, `structure has no horizontal overflow at ${width}px`).toBeLessThanOrEqual(0);
  }

  await expect(page.locator(".pane-drawer .drawer-node .node-label")).toHaveText("Вибрана частина");
  await expect(page.locator(".pane-drawer .drawer-node")).toBeVisible();

  await page.locator("#app-nav [data-app-view='bytes']").click();
  for (const [width, height] of [[390, 844], [768, 1024], [900, 1024]] as const) {
    await page.setViewportSize({ width, height });
    const panes = await page.evaluate(() => {
      const tree = document.querySelector(".pane-tree")!.getBoundingClientRect();
      const bytes = document.querySelector(".pane-hex")!.getBoundingClientRect();
      return { tree, bytes, documentWidth: document.documentElement.scrollWidth };
    });
    expect(panes.bytes.y, `compact bytes view follows the tree at ${width}px`).toBeGreaterThanOrEqual(panes.tree.bottom - 1);
    expect(panes.documentWidth - width, `compact view has no horizontal overflow at ${width}px`).toBeLessThanOrEqual(0);
  }

  await page.setViewportSize({ width: 1100, height: 768 });
  const desktopEdge = await page.evaluate(() => {
    const tree = document.querySelector(".pane-tree")!.getBoundingClientRect();
    const bytes = document.querySelector(".pane-hex")!.getBoundingClientRect();
    const details = document.querySelector(".pane-drawer")!.getBoundingClientRect();
    return { tree, bytes, details };
  });
  expect(desktopEdge.bytes.y).toBeCloseTo(desktopEdge.tree.y, 0);
  expect(desktopEdge.details.y).toBeCloseTo(desktopEdge.tree.y, 0);
  expect(desktopEdge.details.x).toBeGreaterThan(desktopEdge.bytes.x);
});

test("tablet overview adapts its photo, metadata and bottom navigation as one layout", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "tablet analyzer layout is checked on desktop Chromium");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.setViewportSize({ width: 768, height: 900 });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();

  const widths = [520, 560, 600, 620, 640, 659, 660, 680, 700, 768, 900, 901, 1024, 1099];
  await page.locator("#app-nav [data-app-view='metadata']").click();

  for (const width of widths) {
    await page.setViewportSize({ width, height: 900 });
    const groups = page.locator("#drawer .metadata-groups");
    const layout = await groups.evaluate((element) => getComputedStyle(element).gridTemplateColumns.trim().split(/\s+/).map(Number.parseFloat));
    expect(Math.min(...layout), `metadata track width at ${width}px`).toBeGreaterThanOrEqual(320);
    if (width <= 680) expect(layout, `metadata stay in one column at ${width}px`).toHaveLength(1);
    if (width >= 768) expect(layout, `metadata use two readable columns at ${width}px`).toHaveLength(2);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth), `no metadata overflow at ${width}px`).toBeLessThanOrEqual(0);

    const navLayout = await page.evaluate(() => {
      const nav = document.querySelector<HTMLElement>("#app-nav")!;
      const layout = document.querySelector<HTMLElement>(".layout")!;
      return {
        position: getComputedStyle(nav).position,
        navHeight: nav.getBoundingClientRect().height,
        reservedBottom: Number.parseFloat(getComputedStyle(layout).paddingBottom),
      };
    });
    if (width <= 900) {
      expect(navLayout.position, `bottom navigation remains fixed at ${width}px`).toBe("fixed");
      expect(navLayout.reservedBottom, `layout reserves the full navigation height at ${width}px`).toBeGreaterThanOrEqual(navLayout.navHeight - 1);
    } else {
      expect(navLayout.position, `desktop navigation returns to the side rail at ${width}px`).not.toBe("fixed");
    }
  }

  await page.locator("#app-nav [data-app-view='content']").click();
  await expect(page.locator("#drawer .reveal-hero.has-map .place-map-svg")).toBeVisible();
  for (const width of widths) {
    await page.setViewportSize({ width, height: 900 });
    const hero = await page.locator("#drawer .reveal-hero.has-map").evaluate((element) => {
      const picture = element.querySelector(".reveal-picture-frame")!.getBoundingClientRect();
      const map = element.querySelector(".place-map")!.getBoundingClientRect();
      return {
        picture: { x: picture.x, y: picture.y, width: picture.width, bottom: picture.bottom },
        map: { x: map.x, y: map.y, width: map.width },
        clientWidth: element.clientWidth,
        scrollWidth: element.scrollWidth,
      };
    });
    if (width <= 659) {
      expect(hero.map.y, `map follows the full-width photo at ${width}px`).toBeGreaterThanOrEqual(hero.picture.bottom + 8);
      expect(hero.picture.width, `stacked photo remains contained at ${width}px`).toBeLessThanOrEqual(480);
    } else {
      expect(hero.picture.width, `photo width at ${width}px`).toBeGreaterThanOrEqual(240);
      expect(hero.map.width, `map width at ${width}px`).toBeGreaterThanOrEqual(280);
      expect(Math.abs(hero.picture.y - hero.map.y), `photo and map share a row at ${width}px`).toBeLessThanOrEqual(1);
    }
    expect(hero.scrollWidth - hero.clientWidth, `content hero does not overflow at ${width}px`).toBeLessThanOrEqual(0);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth), `no content overflow at ${width}px`).toBeLessThanOrEqual(0);
  }
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
  const inspector = page.locator("dialog.inspector-sheet");
  if (await inspector.isVisible()) await inspector.getByRole("button", { name: "Закрити" }).click();
  await page.locator("#app-nav [data-app-view='bytes']").click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  await expect(page.locator("#hex")).toBeVisible();
});

test("comparison gives both files a clear summary and keeps raw bytes in a detail view", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();

  const desktopCompare = page.locator("#app-nav [data-app-action='compare']");
  if (await desktopCompare.isVisible()) {
    const chooserEvent = page.waitForEvent("filechooser");
    await desktopCompare.click();
    await (await chooserEvent).setFiles(sample("report.docx"));
  } else {
    await page.locator("#drawer .more-details > summary").click();
    const chooserEvent = page.waitForEvent("filechooser");
    await page.locator("#drawer .compare-pick").click();
    await (await chooserEvent).setFiles(sample("report.docx"));
  }

  const dialog = page.locator("dialog.compare");
  await expect(dialog).toBeVisible();
  await expect(dialog).toHaveAttribute("aria-labelledby", "compare-title");
  await expect(dialog.getByRole("heading", { name: "Порівняння" })).toBeVisible();
  const files = dialog.locator(".compare-pair .compare-file");
  await expect(files).toHaveCount(2);
  await expect(files.nth(0)).toContainText("photo.jpg");
  await expect(files.nth(1)).toContainText("report.docx");
  await expect(files.nth(0).locator(".compare-preview canvas")).toBeVisible();
  await expect(files.nth(0).locator(".compare-file-label")).toHaveText("Файл A");
  await expect(files.nth(1).locator(".compare-file-label")).toHaveText("Файл B");
  await expect(files.nth(0).locator(".compare-file-fields")).toContainText("Поля метаданих");
  await expect(dialog.locator(".compare-summary .compare-metric")).toHaveCount(3);
  await expect(dialog.locator(".compare-summary")).toContainText("Знахідки лише у файлі A");
  await expect(dialog.locator(".compare-findings")).toBeVisible();
  await expect(dialog.locator(".compare-findings h3")).toHaveText("Що розкривають файли");
  const firstFinding = dialog.locator(".compare-finding-column").first();
  await firstFinding.locator("summary").click();
  await expect(firstFinding).toContainText("Де зроблено знімок");
  await expect(dialog.locator(".compare-problems")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Закрити порівняння" })).toBeVisible();

  const bounds = await dialog.boundingBox();
  expect(bounds).not.toBeNull();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(page.viewportSize()!.width);
  await expect(page.locator("html")).toHaveJSProperty("scrollWidth", page.viewportSize()!.width);

  const bytes = dialog.locator("details.compare-byte-detail");
  await expect(bytes).toHaveCount(1);
  await expect(bytes).not.toHaveAttribute("open", "");
  await bytes.locator("summary").click();
  await expect(bytes.locator(".compare-rows")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
});

test("phone analyzer opens on overview and reaches bytes through workspace navigation", async ({ page }) => {
  test.skip((page.viewportSize()?.width ?? 1000) > 900, "the desktop uses the full workspace navigation");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  const nav = page.getByRole("navigation", { name: "Робоча область файла" });
  await expect(nav).toBeVisible();
  await expect(page.locator("#viewswitch")).toBeHidden();
  await nav.getByRole("button", { name: "Байти", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  await expect(page.locator("#hex")).toBeVisible();
  await expect(page.locator("html")).toHaveJSProperty("scrollWidth", page.viewportSize()!.width);
  await nav.getByRole("button", { name: "Огляд", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "overview");
});

test("phone analyzer exposes all workspace views and secondary actions", async ({ page }) => {
  test.skip((page.viewportSize()?.width ?? 1000) > 900, "the desktop uses the full workspace navigation");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();

  const nav = page.getByRole("navigation", { name: "Робоча область файла" });
  await expect(nav).toBeVisible();
  await expect(page.locator("#viewswitch")).toBeHidden();
  for (const label of ["Огляд", "Метадані", "Вміст", "Структура", "Байти"]) {
    await expect(nav.getByRole("button", { name: label, exact: true })).toBeVisible();
  }

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
  await expect(page.locator("#hex")).toBeVisible();
  await nav.getByRole("button", { name: "Огляд", exact: true }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "overview");

  const more = nav.locator(".app-nav-more");
  await more.locator("summary").click();
  await expect(more.getByRole("button", { name: "Стиснення", exact: true })).toBeVisible();
  const compare = more.getByRole("button", { name: "Порівняння", exact: true });
  await expect(compare).toBeVisible();
  const navBox = await nav.boundingBox();
  expect(navBox).not.toBeNull();
  expect(navBox!.x).toBeGreaterThanOrEqual(0);
  expect(navBox!.x + navBox!.width).toBeLessThanOrEqual(page.viewportSize()!.width);
  await expect(page.locator("html")).toHaveJSProperty("scrollWidth", page.viewportSize()!.width);

  const comparePicker = page.waitForEvent("filechooser");
  await compare.click();
  await comparePicker;
  await expect(more).not.toHaveAttribute("open", "");

  await page.setViewportSize({ width: 320, height: 740 });
  await expect(page.locator("html")).toHaveJSProperty("scrollWidth", 320);
  for (const label of ["Огляд", "Метадані", "Вміст", "Структура", "Байти"]) {
    const button = nav.getByRole("button", { name: label, exact: true });
    const dimensions = await button.evaluate((element) => ({ width: element.clientWidth, content: element.scrollWidth }));
    expect(dimensions.content).toBeLessThanOrEqual(dimensions.width);
  }
});

test("phone tree selection opens a modal inspector sheet with three clear ways out", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the inspector sheet is specific to touch screens");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await expect(page.locator("dialog.inspector-sheet")).not.toBeVisible();
  await page.locator("#app-nav [data-app-view='structure']").click();

  const firstRow = page.locator("#tree .tree .row").first();
  await firstRow.click();
  const sheet = page.getByRole("dialog", { name: "Вибрана частина" });
  await expect(sheet).toBeVisible();
  await expect(sheet).toHaveAttribute("aria-modal", "true");
  await expect(sheet.locator(".drawer-node")).toContainText("Зсув");
  await expect(sheet.getByRole("button", { name: "Закрити" })).toBeVisible();
  await expect(page.locator("html")).toHaveJSProperty("scrollWidth", 390);

  await sheet.getByRole("button", { name: "Закрити" }).click();
  await expect(sheet).not.toBeVisible();
  await expect(page.locator("#drawer > .drawer-node")).toHaveCount(1);
  await firstRow.click();
  await expect(sheet).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(sheet).not.toBeVisible();

  await firstRow.click();
  await expect(sheet).toBeVisible();
  await page.mouse.click(4, 4);
  await expect(sheet).not.toBeVisible();
  await expect(firstRow).toHaveClass(/is-selected/);
});

test("phone inspector survives crossing the desktop breakpoint and returns to the drawer", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the inspector sheet is specific to touch screens");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator("#app-nav [data-app-view='structure']").click();
  const selectedRow = page.locator("#tree .tree .row").first();
  await selectedRow.click();

  const sheet = page.locator("dialog.inspector-sheet");
  await expect(sheet).toBeVisible();
  await page.setViewportSize({ width: 901, height: 844 });
  await expect(sheet).not.toBeVisible();
  await expect(page.locator("#drawer > .drawer-node")).toHaveCount(1);

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(sheet).toBeVisible();
  await expect(sheet.locator(".drawer-node")).toContainText("Зсув");
  await expect(selectedRow).toHaveClass(/is-selected/);
});

test("phone inspector reopens when selection changes before the native close event", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the inspector sheet is specific to touch screens");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator("#app-nav [data-app-view='structure']").click();
  const rows = page.locator("#tree .tree .row");
  await rows.first().click();

  const sheet = page.locator("dialog.inspector-sheet");
  await expect(sheet).toBeVisible();
  await page.evaluate(() => {
    document.querySelector<HTMLDialogElement>("dialog.inspector-sheet")!.close();
    document.querySelectorAll<HTMLElement>("#tree .tree .row")[1].click();
  });
  await expect(sheet).toBeVisible();
  await expect(sheet.locator(".drawer-node")).toContainText("Зсув");
});

test("phone inspector keeps its request through rapid queued breakpoint closes", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the inspector sheet is specific to touch screens");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
    const matchMedia = window.matchMedia.bind(window);
    window.matchMedia = (query) => {
      const media = matchMedia(query);
      if (query === "(max-width: 900px)") (window as Window & { __inspectorMedia?: MediaQueryList }).__inspectorMedia = media;
      return media;
    };
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator("#app-nav [data-app-view='structure']").click();
  await page.locator("#tree .tree .row").first().click();

  const sheet = page.locator("dialog.inspector-sheet");
  await expect(sheet).toBeVisible();
  await page.evaluate(async () => {
    const media = (window as Window & { __inspectorMedia?: MediaQueryList }).__inspectorMedia;
    const dialog = document.querySelector<HTMLDialogElement>("dialog.inspector-sheet")!;
    if (!media) throw new Error("The phone breakpoint media query was not captured");
    await new Promise<void>((resolve) => {
      let closes = 0;
      dialog.addEventListener("close", () => {
        if (++closes === 2) resolve();
      });
      media.dispatchEvent(new MediaQueryListEvent("change", { media: media.media, matches: false }));
      media.dispatchEvent(new MediaQueryListEvent("change", { media: media.media, matches: true }));
      media.dispatchEvent(new MediaQueryListEvent("change", { media: media.media, matches: false }));
    });
  });
  await expect(sheet).not.toBeVisible();
  await page.evaluate(() => {
    (window as Window & { __inspectorMedia?: MediaQueryList }).__inspectorMedia!.dispatchEvent(
      new MediaQueryListEvent("change", { media: "(max-width: 900px)", matches: true }),
    );
  });
  await expect(sheet).toBeVisible();
});

test("phone inspector keeps a newer selection when explicit close races with resizing", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the inspector sheet is specific to touch screens");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
    const matchMedia = window.matchMedia.bind(window);
    window.matchMedia = (query) => {
      const media = matchMedia(query);
      if (query === "(max-width: 900px)") (window as Window & { __inspectorMedia?: MediaQueryList }).__inspectorMedia = media;
      return media;
    };
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator("#app-nav [data-app-view='structure']").click();
  await page.locator("#tree .tree .row").first().click();

  const sheet = page.locator("dialog.inspector-sheet");
  await expect(sheet).toBeVisible();
  await page.evaluate(async () => {
    const media = (window as Window & { __inspectorMedia?: MediaQueryList }).__inspectorMedia;
    const dialog = document.querySelector<HTMLDialogElement>("dialog.inspector-sheet")!;
    const nextRow = document.querySelectorAll<HTMLElement>("#tree .tree .row")[1];
    if (!media) throw new Error("The phone breakpoint media query was not captured");
    const closeEvents = new Promise<void>((resolve) => {
      let closes = 0;
      dialog.addEventListener("close", () => {
        if (++closes === 2) resolve();
      });
      dialog.querySelector<HTMLButtonElement>(".dialog-x")!.click();
      nextRow.click();
    });
    await Promise.resolve();
    media.dispatchEvent(new MediaQueryListEvent("change", { media: media.media, matches: false }));
    await closeEvents;
  });
  await expect(sheet).not.toBeVisible();
  await page.evaluate(() => {
    (window as Window & { __inspectorMedia?: MediaQueryList }).__inspectorMedia!.dispatchEvent(
      new MediaQueryListEvent("change", { media: "(max-width: 900px)", matches: true }),
    );
  });
  await expect(sheet).toBeVisible();
});

test("phone compression action opens and closes the existing player from More", async ({ page }) => {
  test.skip((page.viewportSize()?.width ?? 1000) > 900, "the desktop exposes compression in its full navigation");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=sample.png");
  await expect(page.locator("body")).toHaveAttribute("data-state", "ready");
  await expect(page.locator("body")).toHaveClass(/is-playing/);

  const nav = page.locator("#app-nav");
  const more = nav.locator(".app-nav-more");
  await more.locator("summary").click();
  const compression = more.getByRole("button", { name: "Стиснення", exact: true });
  await expect(compression).toBeEnabled();
  await expect(compression).toHaveAttribute("aria-pressed", "true");
  await compression.click();
  await expect(page.locator("body")).not.toHaveClass(/is-playing/);
  await expect(page.locator(".drawer-player")).toBeHidden();
  await expect(more).not.toHaveAttribute("open", "");
  await more.locator("summary").click();
  await compression.click();
  await expect(page.locator("body")).toHaveClass(/is-playing/);
  await expect(page.locator(".drawer-player")).toBeVisible();
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
  expect(Math.abs((menuBox!.x + menuBox!.width) - (button!.x + button!.width))).toBeLessThanOrEqual(1);
  expect(menuBox!.x).toBeGreaterThanOrEqual(8);
  expect(menuBox!.y).toBeGreaterThanOrEqual(headerBox!.y + headerBox!.height + 4);
  expect(menuBox!.x + menuBox!.width).toBeLessThanOrEqual(390);
});

/** Opens a sample from its door on the landing page. */
async function openDoor(page: Page, door: RegExp): Promise<void> {
  await home(page);
  const exampleId = /document/i.test(door.source) ? "example-document" : /email/i.test(door.source) ? "example-email" : "example-photo";
  const example = page.locator(`#${exampleId}`);
  if (/check an? (photo|document|email)/i.test(door.source) && (await example.count()) > 0) {
    await page.locator(`label[for="${exampleId}"]`).click();
    await page.locator("#examples [data-example-panel]:visible button[data-sample]").click();
    await expect(page.locator(".verdict-title")).toBeVisible();
    return;
  }
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
  await expect(page.locator(".demo-kicker")).toHaveText("Sample file · example metadata");
  await expect(page.locator(".demo-bytes")).toHaveCount(0);
});

test("the phone demo keeps its copy readable in both languages", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce", colorScheme: "light" });
  await home(page);
  await expect(page.locator(".demo-caption")).toBeVisible();
  await expect(page.locator(".demo-kicker")).toHaveText("Sample file · example metadata");
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

test("the example tabs share one aligned preview on desktop", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "the desktop arrangement is checked at desktop widths");
  await home(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  const grid = page.locator(".doors-main");
  await expect(grid).toHaveCount(1);
  const choices = grid.locator(".example-tabs .door");
  await expect(choices).toHaveCount(3);
  const bounds = await choices.evaluateAll((elements) => elements.map((e) => e.getBoundingClientRect().toJSON()));
  expect(new Set(bounds.map((box) => Math.round(box.y))).size).toBe(1);
  expect(new Set(bounds.map((box) => Math.round(box.height))).size).toBe(1);
  await expect(grid.locator(".example-preview [data-example-panel]:visible")).toHaveCount(1);
});

test("the desktop example tabs line up their labels and file types", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "the card row is checked in its desktop arrangement");
  await home(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  const rows = await page.locator(".example-tabs .door").evaluateAll((tabs) =>
    [".door-title", ".example-tab-type"].map((selector) =>
      tabs.map((tab) => Math.round(tab.querySelector(selector)!.getBoundingClientRect().top)),
    ),
  );
  for (const row of rows) expect(new Set(row).size).toBe(1);
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
    example: parseFloat(getComputedStyle(document.querySelector(".example-copy > p:not(.example-kicker)")!).fontSize),
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

test("the example selector and preview stay inside the content grid across widths and locales", async ({ page }) => {
  await home(page);
  for (const locale of ["en", "uk"] as const) {
    await page.evaluate((language) => localStorage.setItem("hexscope.language", language), locale);
    await page.reload();
    await page.waitForFunction(() => document.body.dataset.state === "empty");
    await expect(page.locator("html")).toHaveAttribute("lang", locale);

    for (const width of [768, 901, 1024, 1034, 1180, 1280, 1440, 1920]) {
      await page.setViewportSize({ width, height: 900 });
      const layout = await page.locator(".doors-main").evaluate((grid) => {
        const frame = grid.getBoundingClientRect();
        const parts = [...grid.querySelectorAll(".example-tabs .door, .example-preview")].map((card) => {
          const bounds = card.getBoundingClientRect();
          return {
            left: bounds.left,
            right: bounds.right,
            width: bounds.width,
          };
        });
        return {
          frame: { left: frame.left, right: frame.right, width: frame.width },
          parts,
          overflow: document.documentElement.scrollWidth - innerWidth,
          viewport: innerWidth,
        };
      });
      expect(layout.parts, `${locale} at ${width}px: three choices and one shared preview`).toHaveLength(4);
      expect(layout.overflow, `${locale} at ${width}px: page overflow`).toBeLessThanOrEqual(0);
      for (const [index, part] of layout.parts.entries()) {
        expect(part.left, `${locale} at ${width}px: item ${index + 1} left edge`).toBeGreaterThanOrEqual(layout.frame.left - 1);
        expect(part.right, `${locale} at ${width}px: item ${index + 1} right edge inside grid`).toBeLessThanOrEqual(layout.frame.right + 1);
        expect(part.left, `${locale} at ${width}px: item ${index + 1} left edge inside viewport`).toBeGreaterThanOrEqual(-1);
        expect(part.right, `${locale} at ${width}px: item ${index + 1} right edge inside viewport`).toBeLessThanOrEqual(layout.viewport + 1);
      }
    }
  }
});

test("the landing groups film scans and local multi-photo actions with their right sections", async ({ page }) => {
  await home(page);
  const tools = page.locator(".film-showcase");
  const privacy = page.locator(".privacy-cta");
  await expect(tools).toBeVisible();
  await expect(tools.getByRole("heading", { name: "Film scans" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Process film scans · JPEG / TIFF" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Open frame AS11-40-5903 in Hexscope →" })).toBeVisible();
  await expect(privacy.getByRole("button", { name: "Choose multiple photos for a privacy mosaic →" })).toHaveAttribute("data-opens", "picker-empty");
  await expect(page.locator(".doors-main #film-roll-open")).toHaveCount(0);
  await expect(page.locator(".doors-main [data-name='film-negative.png']")).toHaveCount(0);

  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await expect(tools.getByRole("heading", { name: "Плівкові скани" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Обробити плівкові скани · JPEG / TIFF" })).toBeVisible();
  await expect(tools.getByRole("button", { name: "Відкрити кадр AS11-40-5903 у Hexscope →" })).toBeVisible();
  await expect(privacy.getByRole("button", { name: "Вибрати кілька фото для мозаїки приватності →" })).toHaveAttribute("data-opens", "picker-empty");

  await page.getByRole("button", { name: "Switch language to English" }).click();
  const fileChooserEvent = page.waitForEvent("filechooser");
  await privacy.getByRole("button", { name: "Choose multiple photos for a privacy mosaic →" }).click();
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

test("the desktop summary reads as one full-width flow at every desktop size", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "desktop summary is checked at desktop widths");
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await expect(page.locator(".film-scan-card")).toBeVisible();

  await expect(page.locator(".overview-answer > .verdict")).toHaveCount(1);
  await expect(page.locator(".overview-flow > .reveals")).toHaveCount(1);
  await expect(page.locator(".overview-flow > .film-scan-card")).toHaveCount(1);
  await expect(page.locator(".overview-flow > .more-details")).toHaveCount(1);

  for (const width of [1100, 1280, 1440, 1920]) {
    await page.setViewportSize({ width, height: 900 });
    const layout = await page.evaluate(() => {
      const file = document.querySelector<HTMLElement>(".drawer-file")!;
      const answer = document.querySelector<HTMLElement>(".overview-answer")!;
      const verdict = document.querySelector<HTMLElement>(".verdict")!;
      const intro = document.querySelector<HTMLElement>(".verdict-intro")!;
      const findings = document.querySelector<HTMLElement>(".verdict-findings")!;
      const flow = document.querySelector<HTMLElement>(".overview-flow")!;
      const film = document.querySelector<HTMLElement>(".film-scan-card")!;
      const answerBox = answer.getBoundingClientRect();
      const introBox = intro.getBoundingClientRect();
      const findingsBox = findings.getBoundingClientRect();
      const flowBox = flow.getBoundingClientRect();
      const filmBox = film.getBoundingClientRect();
      return {
        columns: getComputedStyle(file).gridTemplateColumns.trim().split(/\s+/).length,
        verdictColumns: getComputedStyle(verdict).gridTemplateColumns.trim().split(/\s+/).length,
        answerX: answerBox.x,
        answerBottom: answerBox.bottom,
        answerWidth: answerBox.width,
        introX: introBox.x,
        findingsX: findingsBox.x,
        flowX: flowBox.x,
        flowTop: flowBox.top,
        flowWidth: flowBox.width,
        filmX: filmBox.x,
        filmWidth: filmBox.width,
        overflow: document.documentElement.scrollWidth - innerWidth,
      };
    });
    expect(layout.columns, `${width}px: one page-wide reading flow`).toBe(1);
    expect(layout.verdictColumns, `${width}px: the answer is balanced inside its card`).toBe(2);
    expect(layout.findingsX, `${width}px: findings follow the verdict summary`).toBeGreaterThan(layout.introX);
    expect(layout.flowX, `${width}px: evidence aligns under the answer`).toBeCloseTo(layout.answerX, 0);
    expect(layout.flowTop, `${width}px: evidence follows the answer`).toBeGreaterThan(layout.answerBottom);
    expect(layout.flowWidth, `${width}px: sections share a width`).toBeCloseTo(layout.answerWidth, 0);
    expect(layout.filmX, `${width}px: film clues stay in the same flow`).toBeCloseTo(layout.flowX, 0);
    expect(layout.filmWidth, `${width}px: film clues span the evidence column`).toBeCloseTo(layout.flowWidth, 0);
    expect(layout.overflow, `${width}px: no horizontal page scroll`).toBeLessThanOrEqual(0);
  }
});

test("the first-run tour stays above the fixed bottom navigation", async ({ page }) => {
  await page.setViewportSize({ width: 620, height: 739 });
  await page.addInitScript(() => localStorage.removeItem("hexscope.tour"));
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".tour")).toBeVisible();
  await page.locator(".tour .btn-primary").click();
  await expect(page.locator(".tour")).toBeVisible();
  await expect(page.locator("#app-nav")).toBeVisible();

  const layout = await page.evaluate(() => {
    const bubble = document.querySelector<HTMLElement>(".tour")!.getBoundingClientRect();
    const nav = document.querySelector<HTMLElement>("#app-nav")!;
    const navBox = nav.getBoundingClientRect();
    return {
      bubbleTop: bubble.top,
      bubbleBottom: bubble.bottom,
      navTop: navBox.top,
      navPosition: getComputedStyle(nav).position,
    };
  });
  expect(layout.navPosition).toBe("fixed");
  expect(layout.bubbleTop).toBeGreaterThanOrEqual(8);
  expect(layout.bubbleBottom).toBeLessThanOrEqual(layout.navTop - 8);
});

test("the phone bytes view reserves room for reading the bytes", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the compact tree is specific to touch screens");
  await openDoor(page, /Check a photo/);
  await page.locator("#app-nav [data-app-view='bytes']").click();
  const treeHeight = await page.locator(".pane-tree").evaluate((e) => e.getBoundingClientRect().height);
  expect(treeHeight).toBeLessThanOrEqual((await page.evaluate(() => innerHeight)) * 0.2);
});

test("the phone can inspect a photo in Ukrainian and return to its clean-copy action", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the narrow inspection flow is phone-specific");
  await page.setViewportSize({ width: 320, height: 780 });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.getByRole("button", { name: "Перемкнути мову на українську" }).click();
  await page.locator("#app-nav [data-app-view='bytes']").click();
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
  const inspector = page.getByRole("dialog", { name: "Вибрана частина" });
  await expect(inspector).toBeVisible();
  await expect(inspector.locator(".drawer-node .node-label")).toHaveText("Вибрана частина");
  await expect(inspector.locator(".drawer-node")).toContainText("Початок зображення: перші два байти кожного JPEG.");
  await expect(inspector.locator(".drawer-node")).toContainText("Зсув");
  await expect(inspector.locator(".drawer-node")).toContainText("Довжина");
  await expect(inspector.locator(".drawer-node")).toContainText("Тип");
  await expect(inspector.locator(".drawer-node")).toContainText("Копіювати як");
  const copyBottom = await inspector.locator(".copy-bytes").evaluate((el) => Math.ceil(el.getBoundingClientRect().bottom));
  const drawerBottom = await inspector.locator(".drawer-node").evaluate((el) => Math.floor(el.getBoundingClientRect().bottom));
  expect(copyBottom).toBeLessThanOrEqual(drawerBottom);
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);

  await inspector.getByRole("button", { name: "Закрити" }).click();
  await page.locator("#app-nav [data-app-view='overview']").click();
  await expect(page.locator(".verdict-cta")).toBeVisible();
});

test("the byte grid lets a keyboard user move to and pin the exact byte", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator("#app-nav [data-app-view='bytes']").click();
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

test("the landing page has a clear primary action, three sample choices, and a remembered Ukrainian choice", async ({ page }) => {
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
  await page.locator('label[for="example-photo"]').click();
  await page.locator('#examples [data-example-panel="photo"] button[data-sample]').click();
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
  await expect(page.locator(".topbar .viewswitch")).toBeHidden();
  await expect(page.locator("#app-nav [data-app-view='bytes']")).toBeVisible();
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
      const answer = root.querySelector(".overview-answer");
      const flow = root.querySelector(".overview-flow");
      const children = flow ? [...flow.children] : [];
      return {
        summaryFirst: root.firstElementChild === answer,
        evidenceFlowSecond: answer?.nextElementSibling === flow,
        evidence: children.findIndex((element) => element.matches(".reveals")),
        redaction: children.findIndex((element) => element.matches(".redactor")),
        details: children.findIndex((element) => element.matches(".more-details")),
      };
    });
    expect(order.summaryFirst, `${example.sample}: summary starts the result`).toBe(true);
    expect(order.evidenceFlowSecond, `${example.sample}: evidence follows the summary`).toBe(true);
    expect(order.evidence, `${example.sample}: evidence starts its own flow`).toBe(0);
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
  await expect(page.locator(".tour")).toHaveCount(0);
  await expect(page.locator(".before-after")).toBeInViewport();
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
    if (path !== "deflate.html") {
      const toc = page.locator(".guide-toc");
      if ((page.viewportSize()?.width ?? 0) > 900) {
        await expect(toc).toBeVisible();
        await expect(toc.getByRole("link")).toHaveCount(await article.locator(":scope > section > h2").count());
      } else {
        await expect(toc).toBeHidden();
      }
    }
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

test("guide articles have a translated desktop contents list without narrowing phones", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  await page.goto("./remove-location-from-photo.html");

  const toc = page.locator(".guide-toc");
  const width = page.viewportSize()!.width;
  if (width > 900) {
    await expect(toc).toBeVisible();
    await expect(toc.getByRole("heading", { name: "На цій сторінці" })).toBeVisible();
    const links = toc.getByRole("link");
    await expect(links).toHaveCount(6);
    await expect(links.nth(0)).toHaveText("Спершу перевірте фото");
    await expect(links.nth(0)).toHaveAttribute("aria-current", "location");
    await links.nth(1).click();
    await expect(page).toHaveURL(/#guide-section-2$/);
    await expect(page.locator("#guide-section-2")).toBeInViewport();
    await expect(links.nth(1)).toHaveAttribute("aria-current", "location");
    await links.nth(2).click();
    await expect(links.nth(2)).toHaveAttribute("aria-current", "location");

    await page.locator(".topbar .language-button").click();
    await expect(toc.getByRole("heading", { name: "On this page" })).toBeVisible();
    await expect(links.nth(0)).toHaveText("Check the photo first");
  } else {
    await expect(toc).toBeHidden();
  }

  await expect(page.locator("html")).toHaveJSProperty("scrollWidth", width);
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
  await page.locator('label[for="example-document"]').click();
  await page.locator('#examples [data-example-panel="document"] button[data-sample]').click();
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
