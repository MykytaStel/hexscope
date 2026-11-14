Warning: truncated output (original token count: 42358)
Total output lines: 3230

// What a person does on the site, in a real browser: open a sample, read
// the answer, save a clean copy, check an email, go offline. Run on a
// computer's screen and a phone's (playwright.config.ts).
import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { openView } from "./views";

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

async function analyzerChunksLoaded(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    performance.getEntriesByType("resource").map(({ name }) => name).filter((name) => name.includes("/assets/analyzer-") && name.endsWith(".js")),
  );
}

/** Use the shared workspace navigation on desktop and phone. */
async function openBytes(page: Page): Promise<void> {
  await openView(page, "bytes");
}

test.describe("DEFLATE player loading", () => {
  test.use({ serviceWorkers: "block" });

  test("loads the player only when playback is requested", async ({ page }, info) => {
    test.skip(info.project.name !== "computer", "the script request check runs on the computer project");
    const scripts: string[] = [];
    page.on("request", (request) => {
      if (request.resourceType() === "script") scripts.push(request.url());
    });

    await page.goto("./?sample=photo.jpg");
    await expect(page.locator(".verdict-title")).toBeVisible();
    await page.waitForLoadState("networkidle");
    expect(scripts.some((url) => /\/player-[\w-]+\.js$/.test(url))).toBe(false);

    await page.goto("./?sample=sample.png");
    await expect(page.locator(".drawer-player")).toBeVisible();
    expect(scripts.some((url) => /\/player-[\w-]+\.js$/.test(url))).toBe(true);
  });

  test("keeps the file open and allows retry when the player chunk fails", async ({ page }, info) => {
    await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
    let failNext = true;
    await page.route("**/player-*.js", (route) => {
      if (failNext) {
        failNext = false;
        return route.abort();
      }
      return route.continue();
    });

    await page.goto("./?sample=sample.png");
    await expect(page.locator(".verdict-title")).toBeVisible();
    const error = page.locator("#player-load-error");
    await expect(error).toBeVisible();
    await expect(error).toContainText("Не вдалося завантажити плеєр стиснення. Спробуйте ще раз.");
    await expect(error.getByRole("button", { name: "Спробувати ще раз" })).toBeVisible();
    await expect(page.locator(".drawer-player")).toHaveCount(0);
    if (info.project.name === "phone") {
      await page.setViewportSize({ width: 320, height: 700 });
      await expect(error).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
      const messageLayout = await error.locator(".player-load-error-message").evaluate((message) => {
        const text = message as HTMLElement;
        const retry = document.querySelector<HTMLElement>("#player-load-error [data-retry-player]")!;
        const messageRect = text.getBoundingClientRect();
        const retryRect = retry.getBoundingClientRect();
        return { scrollWidth: text.scrollWidth, clientWidth: text.clientWidth, messageBottom: messageRect.bottom, retryTop: retryRect.top };
      });
      expect(messageLayout.scrollWidth).toBeLessThanOrEqual(messageLayout.clientWidth);
      expect(messageLayout.messageBottom).toBeLessThanOrEqual(messageLayout.retryTop);
    }

    await error.locator("[data-retry-player]").click();
    await expect(error).toBeHidden();
    await expect(page.locator(".drawer-player")).toBeVisible();
    await page.locator(".player-close").click();
    await expect(error).toBeHidden();
    await expect(page.locator(".drawer-node .problem")).toHaveCount(0);
  });

  test("retry resumes the picture step that was requested", async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem("hexscope.language", "en");
      localStorage.setItem("hexscope.tour", "done");
    });
    let requests = 0;
    await page.route("**/player-*.js", async (route) => {
      requests++;
      if (requests === 1) await route.abort();
      else await route.continue();
    });

    await page.goto("./");
    await page.locator("#picker-empty").setInputFiles(new URL("../public/samples/sample.png", import.meta.url).pathname);
    await expect(page.locator(".verdict-title")).toBeVisible();
    await page.locator(".more-details > summary").click();
    const picture = page.locator(".picture-frame");
    await expect(picture).toBeVisible();
    const bounds = await picture.boundingBox();
    expect(bounds).not.toBeNull();
    await picture.click({ position: { x: bounds!.width * 0.75, y: bounds!.height * 0.75 } });

    const step = page.locator(".picture-caption").getByRole("button", { name: /^Step [\d,]+ in the player$/ });
    await expect(step).toBeVisible();
    const label = await step.innerText();
    const requestedStep = Number(label.match(/Step ([\d,]+)/)?.[1].replaceAll(",", ""));
    expect(requestedStep).toBeGreaterThan(1);
    await step.click();
    const error = page.locator("#player-load-error");
    await expect(error).toBeVisible();
    expect(requests).toBe(1);

    await error.locator("[data-retry-player]").click();
    await expect.poll(() => requests).toBe(2);
    await expect(page.locator(".drawer-player")).toBeVisible();
    await expect(page.locator(".player-counter")).toContainText(`Step ${requestedStep} of`);
  });

  test("does not reopen playback after its file is replaced during a chunk load", async ({ page }, info) => {
    test.skip(info.project.name !== "computer", "the pending chunk replacement route runs on the computer project");
    let intercepted = false;
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    await page.route("**/player-*.js", async (route) => {
      intercepted = true;
      await gate;
      await route.continue();
    });

    let response: Promise<unknown> | null = null;
    try {
      await page.goto("./?sample=sample.png");
      await expect.poll(() => intercepted).toBe(true);
      await page.locator("#picker").setInputFiles(new URL("../public/samples/photo.jpg", import.meta.url).pathname);
      await expect(page.locator(".filename")).toContainText("photo.jpg");
      response = page.waitForResponse((r) => /\/player-[\w-]+\.js$/.test(r.url()));
    } finally {
      release();
    }

    await response;
    await page.waitForLoadState("networkidle");
    await expect(page.locator("body")).not.toHaveClass(/is-playing/);
    await expect(page.locator(".drawer-player")).toHaveCount(0);
  });

  test("Escape cancels a pending player import", async ({ page }, info) => {
    test.skip(info.project.name !== "computer", "the pending player import route runs on the computer project");
    let intercepted = false;
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    await page.route("**/player-*.js", async (route) => {
      intercepted = true;
      await gate;
      await route.continue();
    });

    let response: Promise<unknown> | null = null;
    try {
      await page.goto("./?sample=sample.png");
      await expect.poll(() => intercepted).toBe(true);
      await page.keyboard.press("Escape");
      response = page.waitForResponse((r) => /\/player-[\w-]+\.js$/.test(r.url()));
    } finally {
      release();
    }

    await response;
    await page.waitForLoadState("networkidle");
    await expect(page.locator("body")).not.toHaveClass(/is-playing/);
    await expect(page.locator(".drawer-player")).toHaveCount(0);
  });

  test("changing the workspace view cancels a pending player import", async ({ page }, info) => {
    test.skip(info.project.name !== "computer", "the pending player import route runs on the computer project");
    let intercepted = false;
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    await page.route("**/player-*.js", async (route) => {
      intercepted = true;
      await gate;
      await route.continue();
    });

    let response: Promise<unknown> | null = null;
    try {
      await page.goto("./?sample=sample.png");
      await expect.poll(() => intercepted).toBe(true);
      await page.locator("#app-nav [data-app-view='metadata']").click();
      response = page.waitForResponse((r) => /\/player-[\w-]+\.js$/.test(r.url()));
    } finally {
      release();
    }

    await response;
    await page.waitForLoadState("networkidle");
    await expect(page.locator("body")).not.toHaveClass(/is-playing/);
    await expect(page.locator(".drawer-player")).toHaveCount(0);
    await expect(page.locator("body")).toHaveAttribute("data-app-view", "metadata");
  });
});

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

for (const scheme of ["light", "dark"] as const) {
  test(`automatic theme follows the system's ${scheme} preference`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.addInitScript(() => localStorage.setItem("hexscope.theme", "auto"));
    await page.goto("./");
    await page.waitForFunction(() => document.body.dataset.state === "empty");

    await expect(page.locator("html")).not.toHaveAttribute("data-theme");
    const actual = await page.evaluate(() => ({
      background: getComputedStyle(document.body).backgroundColor,
      hexSelectionAlpha: getComputedStyle(document.documentElement).getPropertyValue("--alpha-base").trim(),
    }));
    expect(actual).toEqual({
      background: scheme === "light" ? "rgb(246, 246, 244)" : "rgb(9, 11, 13)",
      hexSelectionAlpha: scheme === "light" ? ".1" : ".14",
    });
  });
}

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
  const layout = await panel.evaluate((element) => {
    const rect = element.getBoundingClientRect();
    const trigger = element.parentElement!.querySelector(":scope > summary")!.getBoundingClientRect();
    const style = getComputedStyle(element);
    return {
      viewport: { width: innerWidth, clientWidth: document.documentElement.clientWidth },
      panel: { x: rect.x, width: rect.width, right: rect.right, widthCss: style.width, maxWidth: style.maxWidth, minWidth: style.minWidth },
      trigger: { x: trigger.x, width: trigger.width, right: trigger.right },
      items: Array.from(element.children, (item) => ({
        text: item.textContent?.trim(),
        hidden: item.hasAttribute("hidden"),
        display: getComputedStyle(item).display,
        width: item.getBoundingClientRect().width,
      })),
    };
  });
  expect(panelBox!.x, JSON.stringify(layout)).toBeGreaterThanOrEqual(8);
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
  // Small copies, sharp on a 2x screen at the tiles' size; the originals stay samples.
  await expect.poll(() => frames.evaluateAll((images) => images.every((image) => (image as HTMLImageElement).naturalWidth >= 500))).toBe(true);
  expect(await frames.evaluateAll((images) => images.every((image) => (image as HTMLImageElement).currentSrc.includes("/thumbs/")))).toBe(true);
  const tileHeights = await frames.evaluateAll((images) => images.map((image) => image.getBoundingClientRect().height));
  expect(Math.min(...tileHeights)).toBeGreaterThan(120);
  expect(Math.max(...tileHeights)).toBeLessThanOrEqual(240);
  expect((await film.boundingBox())!.height).toBeLessThan(1100);
  const sources = await frames.evaluateAll((images) => images.map((image) => (image as HTMLImageElement).currentSrc));
  expect(new Set(sources).size).toBe(4);
  await expect(film.getByRole("link", { name: /NASA Apollo 11 archive/i })).toHaveAttribute("href", /nasa\.gov/);
  await expect(film).toContainText(/Published positive scans/);
  await expect(film.getByRole("button", { name: "Process film scans · JPEG / TIFF" })).toBeVisible();
  await expect(film.getByRole("button", { name: "Open frame AS11-40-5903 in hexscope →" })).toHaveAttribute(
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
  await openView(page, "bytes");
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
  awai…30358 tokens truncated…toHaveCount(0);
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

test("a phone clipboard permission failure tells the person how to continue", async ({ page }) => {
  await home(page);
  await page.evaluate(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { read: () => Promise.reject(new DOMException("permission denied", "NotAllowedError")) },
    });
    document.querySelector<HTMLButtonElement>("[data-paste]")!.hidden = false;
  });
  await page.locator(".hero-other-ways > summary").click();

  const paste = page.locator("[data-paste]");
  await expect(paste).toBeVisible();
  await paste.click();
  await expect(page.locator("#load-error")).toBeVisible();
  await expect(page.locator("#load-error")).toContainText("choose the file");
  await expect(page.locator("body")).toHaveAttribute("data-state", "empty");
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
    "spf", "dkim", "dmarc", "pass", "eur", "wi", "fi", "qr", "f12", "csp", "css", "xml", "url", "http", "https",
    "rust", "cookie",
    "jbig", "jbig2", "ccitt", "fax", "gzip", "javascript", "crc",
    "libreoffice", "rtf", "exiftool", "mat2", "svg",
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
          .replace(/\bAuthentication-Results\b/gi, " ")
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
  await page.addInitScript(() => {
    if (!localStorage.getItem("hexscope.language")) localStorage.setItem("hexscope.language", "uk");
  });
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
  if (info.project.name === "phone") await page.setViewportSize({ width: 320, height: 844 });
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
    await expect(list.locator(".batch-result")).toContainText("Share the ZIP from Files or Downloads.");
    const result = list.locator(".batch-result");
    await page.locator(".topbar .language-button").click();
    await expect(page.locator("html")).toHaveAttribute("lang", "uk");
    await expect.poll(() => result.evaluate((element) => element.textContent ?? "")).toMatch(/\. У ZIP/);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const actionsRight = await page.locator(".topbar .actions").evaluate((element) => element.getBoundingClientRect().right);
    expect(actionsRight).toBeLessThanOrEqual(320);
  }
});

test("batch copy fingerprints are opt-in and match the bytes stored in the ZIP", async ({ page }, info) => {
  test.skip(info.project.name !== "computer", "the phone offers the ZIP from its share actions");
  await home(page);
  await page.locator("#picker-empty").setInputFiles([sample("photo.jpg"), sample("report.docx")]);
  const option = page.locator(".batch-fingerprints input");
  await expect(option).toBeVisible();
  await expect(option).not.toBeChecked();
  await page.locator(".batch-fingerprints").hover();
  await expect(page.locator(".batch-fingerprints")).toHaveAttribute(
    "title",
    "Direct phone sharing sends copies only; Save as a ZIP includes the receipt and fingerprints.",
  );
  await option.check();

  const downloadPromise = page.waitForEvent("download");
  await page.locator("#batch .btn-clean").click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("hexscope-clean-copies.zip");
  const archive = storedZipEntries(await download.path());
  const outputBytes = [...archive.entries()].find(([name]) => name.endsWith(".jpg"))?.[1];
  const reportBytes = archive.get("hexscope-report.json");
  expect(outputBytes).toBeDefined();
  expect(reportBytes).toBeDefined();
  const reportText = reportBytes!.toString("utf8");
  const report = JSON.parse(reportText);
  expect(report.schema).toBe("hexscope.batch-clean-copy-receipt");
  expect(report.files).toHaveLength(2);
  expect(report.files[0]).toMatchObject({
    index: 1,
    operation_state: "written",
    receipt: {
      schema: "hexscope.clean-copy-receipt",
      copy: { format: "jpeg", sha256: createHash("sha256").update(outputBytes!).digest("hex") },
    },
  });
  expect(reportText).not.toContain("photo.jpg");
  expect(reportText).not.toContain("hexscope-clean-copies.zip");
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
        if (
          urls.filter((u) => u.endsWith(".wasm")).length >= 2 &&
          urls.some((u) => u.endsWith("redacted.pdf")) &&
          urls.some((u) => /pdf-redactor-view-[^/]+\.js$/.test(u)) &&
          urls.some((u) => /report-[^/]+\.js$/.test(u)) &&
          urls.some((u) => /export-[^/]+\.js$/.test(u))
        ) return;
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
  await page.locator("#picker").setInputFiles(sample("report.pdf"));
  await expect(page.locator("#fileinfo .filename")).toHaveText("report.pdf");
  const redactor = page.locator(".redactor");
  await redactor.locator('input[type="search"]').fill("Salary");
  await redactor.getByRole("button", { name: "Find", exact: true }).click();
  await expect(redactor.locator("mark.redact-hit")).toContainText("Salary");
  await page.locator(".more-details > summary").click();
  const report = page.locator("dialog.report");
  await page.getByRole("button", { name: "Report a problem with this file" }).click();
  await expect(report).toBeVisible();
  await report.locator(".report-actions button").last().click();
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Save the structure as JSON" }).click();
  await (await download).cancel();
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
  // Its title and author, and the dates in Word's own record of it.
  await expect(page.locator(".verdict-title")).toHaveText("This document gives away 4 things");
  await expect(page.locator(".reveal-list")).toContainText("Olena Koval");
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-count")).toHaveText("4");
  await expect(page.locator(".ba-side.is-after")).toHaveClass(/is-clear/);
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

test("a link opens a sample at the view and the part it names", async ({ page }) => {
  await page.goto("./?sample=photo.jpg&part=GPS%20IFD");
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  await expect(page.locator("#statusbar-file")).toContainText("GPS IFD");
  await page.goto("./?sample=report.docx&view=metadata");
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "metadata");
  await expect(page.locator(".overview-answer")).toBeHidden();
  await expect(page.locator("#drawer .metadata-groups")).toBeVisible();
});

test("the bytes' tools read the bytes at the cursor, hash the file and go to an offset", async ({ page }) => {
  await page.goto("./?sample=photo.jpg&view=bytes");
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  await page.locator("#hex-tools > summary").click();
  await expect(page.locator(".bytetools-values h3")).toHaveText("At 0x00000000");
  await expect(page.locator(".bytetools-table tr", { hasText: "8-bit" })).toContainText("255 · -1 · 0xFF");
  await page.locator(".bytetools-hash button").first().click();
  // shasum -a 256 apps/web/public/samples/photo.jpg
  await expect(page.locator(".bytetools-hash code").first()).toHaveText("935ab93bc27586f2623d30e9fcc0dd97ce645c7654a742097113ae237be6db4a");
  await expect(page.locator(".bytetools-entropy")).toHaveAttribute("aria-label", /Entropy along the file: \d\.\d \/ 8/);
  await page.locator(".search-open").click();
  await page.locator("#hex-find input").fill("@0x1F0");
  await expect(page.locator(".bytetools-values h3")).toHaveText("At 0x000001F0");
  await expect(page.locator("#status")).toContainText("0x000001F0");
  await page.locator("#hex-find input").fill("@999999999");
  await expect(page.locator(".search-count")).toHaveText("past the end of the file");
});

test("a guide's sample opens where the guide is about", async ({ page }) => {
  await page.goto("./remove-location-from-photo");
  await page.locator('a[href*="sample=photo.jpg"]').first().click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "metadata");
  await expect(page.locator('#drawer .reveal-list dt[data-kind="location"]')).toBeVisible();
});
