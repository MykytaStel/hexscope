// Writing to the developer from the page, without an account, and thanking
// them: what is sent is what the dialog shows, and never the file.
import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
});

test("a message goes from the footer with nothing but what was written and the page's own details", async ({ page }) => {
  const got: { sent: Record<string, string> | null } = { sent: null };
  await page.route("**/api/feedback", async (route) => {
    got.sent = route.request().postDataJSON();
    await route.fulfill({ status: 204 });
  });
  await page.goto("./");
  await page.getByRole("button", { name: "Write to the developer" }).click();
  const dialog = page.locator("dialog.feedback");
  await dialog.getByRole("radio", { name: "An idea" }).click();
  await dialog.getByRole("textbox", { name: "Your message" }).fill("Please read RTF files.");
  await dialog.getByRole("button", { name: "Send" }).click();
  await expect(dialog).toContainText("Sent. Thank you — it will be read.");
  expect(got.sent).toMatchObject({ kind: "idea", message: "Please read RTF files.", report: "", page: "/", lang: "en", website: "" });
  expect(Object.keys(got.sent ?? {}).sort()).toEqual(["contact", "device", "kind", "lang", "message", "page", "report", "version", "website"]);
});

test("a file's structure report goes along only when ticked, and says nothing of what the file says", async ({ page }) => {
  const got: { sent: Record<string, string> | null } = { sent: null };
  await page.route("**/api/feedback", async (route) => {
    got.sent = route.request().postDataJSON();
    await route.fulfill({ status: 204 });
  });
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toBeVisible();
  await page.locator(".more-details > summary").click();
  await page.getByRole("button", { name: "Report a problem with this file" }).click();
  await page.getByRole("button", { name: "Send to the developer" }).click();
  const dialog = page.locator("dialog.feedback");
  await expect(dialog.getByRole("checkbox")).toBeChecked();
  await dialog.getByRole("textbox", { name: "Your message" }).fill("The map is in the wrong place.");
  await dialog.getByRole("button", { name: "Send" }).click();
  await expect(dialog).toContainText("Sent.");
  expect(got.sent?.report).toContain("hexscope structure report");
  // The photo's values are not in it: no place, no serial number, no owner.
  for (const said of ["48.858", "HX-000042", "Sample Owner"]) expect(got.sent?.report).not.toContain(said);
});

test("a message that cannot be sent says so, and offers GitHub instead", async ({ page }) => {
  await page.route("**/api/feedback", (route) => route.fulfill({ status: 503 }));
  await page.goto("./");
  await page.getByRole("button", { name: "Write to the developer" }).click();
  const dialog = page.locator("dialog.feedback");
  await dialog.getByRole("textbox", { name: "Your message" }).fill("Something broke.");
  await dialog.getByRole("button", { name: "Send" }).click();
  await expect(dialog.locator(".feedback-status")).toContainText("Not sent. The site could not take it right now.");
  await expect(dialog.getByRole("link", { name: "Write on GitHub instead ↗" })).toHaveAttribute("href", /github\.com/);
});

test("after the third clean copy, a quiet thanks line shows once, and closes for good", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.cleans", localStorage.getItem("hexscope.cleans") ?? "2"));
  await page.goto("./?sample=photo.jpg");
  await page.locator(".verdict-cta").click();
  const line = page.locator(".thanks-line");
  await expect(line).toContainText("you can thank them");
  await expect(line.getByRole("link")).toHaveAttribute("href", "/support");
  await line.getByRole("button", { name: "Do not show this again" }).click();
  await expect(line).toHaveCount(0);
  expect(await page.evaluate(() => localStorage.getItem("hexscope.thanks-hidden"))).toBe("1");
});

test("the support page offers ways to help, and every guide ends with a way to write", async ({ page }) => {
  await page.goto("./support");
  await expect(page.locator("h1")).toHaveText("Support hexscope");
  await expect(page.getByRole("button", { name: "write here" })).toBeVisible();
  await page.goto("./exif-viewer");
  await expect(page.locator(".story-reach")).toContainText("Write to the developer");
  await expect(page.locator(".story-reach a")).toHaveAttribute("href", "support");
});
