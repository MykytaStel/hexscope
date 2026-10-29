// The Files view of an archive: folders, a filter, an entry's details beside
// the list (a sheet on a phone), and nothing unpacked until asked.
import { expect, test, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { openView } from "./views";
import { zipOf } from "./zips";

const BUNDLE = zipOf([
  { name: "docs/" },
  { name: "docs/report.txt", data: "report" },
  { name: "docs/notes/todo.txt", data: "todo" },
  { name: "photos/holiday.jpg", data: "not a photo" },
  { name: "readme.txt", data: "hello" },
  { name: "Structure", data: "a file named like a view" },
  { name: "secret.txt", data: "xxxx", flags: 1 },
  { name: "odd.bin", data: "xxxx", method: 99 },
  { name: "../../evil.sh", data: "echo" },
  { name: "/etc/passwd", data: "root" },
]);

/** Counts what the page asks the worker for, by kind. */
function countRequests(): void {
  const seen: Record<string, number> = {};
  (window as typeof window & { __requests: Record<string, number> }).__requests = seen;
  const send = Worker.prototype.postMessage;
  Worker.prototype.postMessage = function (this: Worker, message: { type?: string }, ...rest: [StructuredSerializeOptions?]) {
    if (message?.type) seen[message.type] = (seen[message.type] ?? 0) + 1;
    return send.call(this, message, ...rest);
  } as typeof Worker.prototype.postMessage;
}
const requests = (page: Page) => page.evaluate(() => ({ ...(window as typeof window & { __requests: Record<string, number> }).__requests }));

async function start(page: Page, language: "en" | "uk" = "en"): Promise<void> {
  await page.addInitScript((lang) => {
    localStorage.setItem("hexscope.language", lang);
    localStorage.setItem("hexscope.tour", "done");
  }, language);
  await page.addInitScript(countRequests);
  await page.goto("./");
  await page.locator("body[data-state=empty]").waitFor();
}

async function openZip(page: Page, buffer: Buffer, name = "bundle.zip"): Promise<void> {
  await page.locator("#picker-empty").setInputFiles({ name, mimeType: "application/zip", buffer });
  await page.locator("body[data-state=ready]").waitFor();
}

const pane = (page: Page) => page.locator(".pane-files");
const rows = (page: Page) => pane(page).locator("[role=option]");
const row = (page: Page, name: string) => rows(page).filter({ has: page.locator(".archive-name", { hasText: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`) }) });

async function showFiles(page: Page): Promise<void> {
  await openView(page, "files");
  await expect(rows(page).first()).toBeVisible();
}

/** The chosen entry's details: beside the list on a computer, in the sheet on a phone. */
async function details(page: Page, phone: boolean) {
  if (phone) {
    const sheet = page.locator("dialog.inspector-sheet[open]");
    await expect(sheet).toBeVisible();
    const toggle = sheet.locator(".inspector-details-toggle");
    if ((await toggle.getAttribute("aria-expanded")) !== "true") await toggle.click();
    return sheet;
  }
  return page.locator(".pane-drawer");
}

test("an archive opens its files, filters, shows an entry and comes back where it was", async ({ page }, info) => {
  const phone = info.project.name === "phone";
  await start(page);
  await openZip(page, BUNDLE);
  await showFiles(page);
  await expect(pane(page).locator(".archive-summary")).toContainText("9 entries in 7 folders");
  await expect(rows(page).locator(".archive-name")).toHaveText(["..", "/", "docs", "photos", "Structure", "odd.bin", "readme.txt", "secret.txt"]);
  await pane(page).getByLabel("Filter by name").fill("todo");
  await expect(pane(page).locator(".archive-status")).toHaveText("1 entry matches");
  await row(page, "docs/notes/todo.txt").click();
  const where = await details(page, phone);
  await expect(where.locator(".group.entry")).toContainText("docs/notes/todo.txt");
  await where.getByRole("button", { name: "Show compressed data bytes" }).click();
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "bytes");
  if (phone) await page.keyboard.press("Escape");
  await showFiles(page);
  await expect(pane(page).getByLabel("Filter by name")).toHaveValue("todo");
  await expect(row(page, "docs/notes/todo.txt")).toHaveAttribute("aria-selected", "true");
});

test("photos and Office documents have no Files view", async ({ page }) => {
  await start(page);
  for (const sample of ["photo.jpg", "report.docx", "lease.odt"]) {
    await page.goto(`./?sample=${sample}`);
    await page.locator("body[data-state=ready]").waitFor();
    await expect(page.locator("#app-nav [data-app-view='files']")).toBeHidden();
  }
});

test("a large archive keeps the page small, keeps its place, and reads no entry until asked", async ({ page }, info) => {
  test.setTimeout(120_000);
  const phone = info.project.name === "phone";
  await start(page);
  const many = zipOf(Array.from({ length: 20_000 }, (_, i) => ({ name: `area-${String(i % 20).padStart(2, "0")}/file-${i}.txt`, data: `entry ${i}` })));
  await openZip(page, many, "many.zip");
  await showFiles(page);
  expect(await pane(page).locator("*").count()).toBeLessThanOrEqual(400);
  await row(page, "area-00").click();
  const list = pane(page).locator(".archive-list");
  await list.evaluate((box) => { box.scrollTop = box.scrollHeight / 2; });
  await page.waitForTimeout(100);
  expect(await pane(page).locator("*").count()).toBeLessThanOrEqual(400);
  const picked = rows(page).nth(10);
  const name = (await picked.locator(".archive-name").textContent())!;
  await picked.click();
  const top = await list.evaluate((box) => box.scrollTop);
  await pane(page).getByLabel("Filter by name").fill("file-19999.txt");
  await expect(pane(page).locator(".archive-status")).toHaveText("1 entry matches");
  await pane(page).getByLabel("Filter by name").fill("");
  await expect.poll(() => list.evaluate((box) => box.scrollTop)).toBe(top);
  const asked = await requests(page);
  expect(asked.open ?? 0).toBe(0);
  if (phone) await page.keyboard.press("Escape");
  await openView(page, "structure");
  await showFiles(page);
  await expect.poll(() => list.evaluate((box) => box.scrollTop)).toBe(top);
  await expect(row(page, name)).toHaveAttribute("aria-selected", "true");
  await row(page, name).click();
  const where = await details(page, phone);
  await where.locator(".archive-trace .entry-actions").getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.locator(".filename").first()).toContainText(name);
  expect((await requests(page)).open).toBe(1);
});

test("hostile names are shown as written and flagged", async ({ page }) => {
  await start(page);
  await openZip(page, BUNDLE);
  await showFiles(page);
  const up = pane(page).locator(".archive-up");
  await expect(up).toBeDisabled();
  await expect(up).toHaveAttribute("aria-label", "Up to the top level");
  await expect(row(page, "secret.txt").locator(".archive-flag")).toHaveText("Encrypted");
  await expect(row(page, "odd.bin").locator(".archive-flag")).toHaveText("Cannot be unpacked here");
  await expect(row(page, "readme.txt").locator(".archive-flag")).toHaveCount(0);
  await row(page, "..").click();
  await expect(pane(page).locator(".archive-path")).toHaveText("../");
  await expect(up).toBeEnabled();
  await row(page, "..").click();
  await expect(row(page, "evil.sh").locator(".archive-flag")).toHaveText("Unpacks outside its folder");
  await expect(up).toHaveAttribute("aria-label", "Up to “..”");
  await up.click();
  await up.click();
  await row(page, "/").click();
  await row(page, "etc").click();
  await expect(row(page, "passwd").locator(".archive-flag")).toHaveText("Unpacks outside its folder");
});

test("an archive's names stay as written in Ukrainian", async ({ page }) => {
  await start(page, "uk");
  await openZip(page, BUNDLE);
  await expect(page.locator("#app-nav [data-app-view='files']")).toContainText("Файли");
  await showFiles(page);
  await expect(pane(page).locator(".archive-summary")).toContainText("9 записів · 7 тек");
  await expect(row(page, "Structure")).toHaveCount(1);
  await expect(row(page, "secret.txt").locator(".archive-flag")).toHaveText("Зашифровано");
});

test("choosing quickly acts on the last choice", async ({ page }) => {
  test.skip(test.info().project.name === "phone", "the sheet covers the list between choices on a phone");
  await start(page);
  await openZip(page, BUNDLE);
  await showFiles(page);
  await row(page, "readme.txt").click();
  await row(page, "Structure").click();
  const open = page.locator(".pane-drawer .archive-trace .entry-actions").getByRole("button", { name: "Open", exact: true });
  await open.click();
  await expect(page.locator(".filename").first()).toContainText("Structure");
});

test("another file while Files is open goes to its overview", async ({ page }) => {
  await start(page);
  await openZip(page, BUNDLE);
  await showFiles(page);
  await page.locator("#picker").setInputFiles({ name: "note.txt", mimeType: "text/plain", buffer: Buffer.from("just text") });
  await expect(page.locator(".filename").first()).toContainText("note.txt");
  await expect(page.locator("body")).toHaveAttribute("data-app-view", "overview");
  await expect(pane(page)).toBeHidden();
  await expect(page.locator("#app-nav [data-app-view='files']")).toBeHidden();
});

test("the list works by keyboard and passes axe in both themes", async ({ page }) => {
  test.skip(test.info().project.name === "phone", "keyboard use is a computer's");
  await start(page);
  await openZip(page, BUNDLE);
  await showFiles(page);
  const list = pane(page).locator(".archive-list");
  await list.focus();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await expect(pane(page).locator(".archive-row.is-active .archive-name")).toHaveText("docs");
  await page.keyboard.press("Enter");
  await expect(pane(page).locator(".archive-path")).toHaveText("docs/");
  await expect(rows(page).locator(".archive-name")).toHaveText(["notes", "report.txt"]);
  await expect(pane(page).locator(".archive-path")).not.toHaveClass(/is-top/);
  await page.keyboard.press("Backspace");
  await expect(pane(page).locator(".archive-path")).toHaveText("All files");
  // The top is the page's words, not a path the archive wrote.
  await expect(pane(page).locator(".archive-path")).toHaveClass(/is-top/);
  await expect(pane(page).locator(".archive-row.is-active .archive-name")).toHaveText("docs");
  await page.keyboard.press("/");
  await expect(pane(page).getByLabel("Filter by name")).toBeFocused();
  // Up turns off at the top; the keyboard lands on the list, not on nothing.
  await row(page, "docs").click();
  const up = pane(page).locator(".archive-up");
  await up.focus();
  await page.keyboard.press("Enter");
  await expect(up).toBeDisabled();
  await expect(pane(page).locator(".archive-list")).toBeFocused();
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    const result = await new AxeBuilder({ page }).include(".pane-files").analyze();
    expect(result.violations).toEqual([]);
  }
});

test("on a phone the list fits and keeps its place under the sheet", async ({ page }, info) => {
  test.skip(info.project.name !== "phone", "the sheet and the widths are a phone's");
  await start(page);
  await openZip(page, zipOf(Array.from({ length: 200 }, (_, i) => ({ name: `long-folder-name-for-a-narrow-screen/file-number-${i}.txt`, data: "x" }))), "narrow.zip");
  for (const width of [320, 375]) {
    await page.setViewportSize({ width, height: 640 });
    await showFiles(page);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
  }
  await rows(page).first().click();
  const heights = await rows(page).evaluateAll((all) => all.map((r) => r.getBoundingClientRect().height));
  expect(Math.min(...heights)).toBeGreaterThanOrEqual(44);
  const list = pane(page).locator(".archive-list");
  await list.evaluate((box) => { box.scrollTop = 1000; });
  await page.waitForTimeout(100);
  // A row in view: the window draws a few above the edge, and clicking one of those would scroll.
  await pane(page).locator("[role=option][aria-posinset='26']").click();
  await expect(page.locator("dialog.inspector-sheet[open]")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator("dialog.inspector-sheet[open]")).toHaveCount(0);
  expect(await list.evaluate((box) => box.scrollTop)).toBe(1000);
});

test.describe("Files chunk retry", () => {
  test.use({ serviceWorkers: "block" });

  test("a list that could not load says so and loads on a second try", async ({ page }) => {
    // Vite preloads the chunk and then imports it: both requests fail until the network is back.
    let offline = true;
    await page.route("**/archive-explorer-*.js", (route) => (offline ? route.abort() : route.continue()));
    await start(page);
    await openZip(page, BUNDLE);
    await openView(page, "files");
    await expect(pane(page)).toContainText("Could not load the list of files.");
    offline = false;
    await pane(page).getByRole("button", { name: "Try again" }).click();
    await expect(rows(page).first()).toBeVisible();
  });
});
