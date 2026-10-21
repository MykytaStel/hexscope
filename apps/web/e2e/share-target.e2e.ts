import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";

const sample = (name: string) => new URL(`../public/samples/${name}`, import.meta.url).pathname;

async function controlledHome(page: Page): Promise<void> {
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.evaluate(() => navigator.serviceWorker.ready);
  if (!(await page.evaluate(() => Boolean(navigator.serviceWorker.controller)))) {
    await page.reload();
    await page.waitForFunction(() => Boolean(navigator.serviceWorker.controller));
    await page.waitForFunction(() => document.body.dataset.state === "empty");
  }
}

async function submitShare(page: Page, names: string[]): Promise<void> {
  const files = names.map((name) => ({
    name,
    type: name.endsWith(".eml") ? "message/rfc822" : "image/jpeg",
    bytes: [...readFileSync(sample(name))],
  }));
  await page.evaluate((shared) => {
    void (async () => {
      const form = new FormData();
      for (const file of shared) form.append("file", new File([new Uint8Array(file.bytes)], file.name, { type: file.type }));
      const response = await fetch(new URL("share", location.href), { method: "POST", body: form });
      if (!response.ok) throw new Error(`Share POST failed: ${response.status}`);
      location.assign(response.url);
    })();
  }, files);
}

async function seedLegacy(page: Page): Promise<void> {
  const bytes = [...readFileSync(sample("message.eml"))];
  await page.evaluate((contents) =>
    caches.open("hexscope-shared").then((cache) =>
      cache.put(new URL("shared-file", location.href), new Response(new Uint8Array(contents), { headers: { "x-file-name": "message.eml" } })),
    ),
    bytes,
  );
}

async function seedExpiredSubmission(page: Page, id: string): Promise<void> {
  const bytes = [...readFileSync(sample("message.eml"))];
  await page.evaluate(({ contents, submission }) =>
    caches.open("hexscope-shared").then((cache) =>
      cache.put(new URL(`shared/${submission}/0`, location.href), new Response(new Uint8Array(contents), { headers: {
        "x-submission-id": submission,
        "x-file-index": "0",
        "x-file-count": "1",
        "x-file-name": "message.eml",
        "x-file-type": "message/rfc822",
        "x-received-at": String(Date.now() - 25 * 60 * 60 * 1000),
      } })),
    ),
    { contents: bytes, submission: id },
  );
}

async function seedIncompleteSubmission(page: Page, id: string): Promise<void> {
  const bytes = [...readFileSync(sample("message.eml"))];
  await page.evaluate(({ contents, submission }) =>
    caches.open("hexscope-shared").then((cache) =>
      cache.put(new URL(`shared/${submission}/0`, location.href), new Response(new Uint8Array(contents), { headers: {
        "x-submission-id": submission,
        "x-file-index": "0",
        "x-file-count": "2",
        "x-file-name": "message.eml",
        "x-file-type": "message/rfc822",
        "x-received-at": String(Date.now()),
      } })),
    ),
    { contents: bytes, submission: id },
  );
}

test("a single shared file opens the ordinary analyzer", async ({ page }) => {
  await controlledHome(page);
  await submitShare(page, ["message.eml"]);
  await expect(page.locator(".verdict-title")).toHaveText(/^This email gives away \d+ things$/);
  await expect(page).not.toHaveURL(/shared=/);
});

test("multiple shared files open together in the batch analyzer", async ({ page }) => {
  await controlledHome(page);
  await submitShare(page, ["message.eml", "photo.jpg"]);
  const batch = page.locator("#batch");
  await expect(batch).toContainText("message.eml");
  await expect(batch).toContainText("photo.jpg");
  await expect(page.locator(".batch-title")).toHaveText("2 of 2 files need a look");
  await expect(page).not.toHaveURL(/shared=/);
});

test("a legacy shared-file entry is still opened once", async ({ page }) => {
  await controlledHome(page);
  await seedLegacy(page);
  await page.goto("./?shared=1");
  await expect(page.locator(".verdict-title")).toHaveText(/^This email gives away \d+ things$/);
  await expect(page).not.toHaveURL(/shared=/);
  await expect.poll(() => page.evaluate(async () => (await (await caches.open("hexscope-shared")).match("shared-file")) === undefined)).toBe(true);
});

test("a missing or expired submission explains how to share the file again", async ({ page }) => {
  await controlledHome(page);
  await page.goto("./?shared=00000000-0000-4000-8000-000000000011");
  await expect(page.locator("#load-error")).toBeVisible();
  await expect(page.locator("#load-error")).toHaveText("This shared file is no longer available. Share it again from the app that has it.");
  await expect(page).not.toHaveURL(/shared=/);

  await seedExpiredSubmission(page, "00000000-0000-4000-8000-000000000012");
  await page.goto("./?shared=00000000-0000-4000-8000-000000000012");
  await expect(page.locator("#load-error")).toBeVisible();
  await expect(page.locator("#load-error")).toContainText("Share it again");
});

test("does not open an expired submission when cache cleanup fails", async ({ page }) => {
  const id = "00000000-0000-4000-8000-000000000014";
  await page.addInitScript((submission) => {
    const remove = Cache.prototype.delete;
    let failed = false;
    Cache.prototype.delete = function (request, options) {
      const url = request instanceof Request ? request.url : String(request);
      if (!failed && new URL(url, location.href).pathname.includes(`/shared/${submission}/`)) {
        failed = true;
        document.documentElement.setAttribute("data-cache-delete-failed", "true");
        return Promise.reject(new Error("simulated cache deletion failure"));
      }
      return remove.call(this, request, options);
    };
  }, id);
  await controlledHome(page);
  await seedExpiredSubmission(page, id);
  await page.goto(`./?shared=${id}`);
  await expect(page.locator("#load-error")).toBeVisible();
  await expect(page.locator("#load-error")).toContainText("Share it again");
  await expect(page.locator(".verdict-title")).toHaveCount(0);
  await expect(page.locator("html")).toHaveAttribute("data-cache-delete-failed", "true");
});

test("an invalid submission id cannot consume the legacy entry", async ({ page }) => {
  await controlledHome(page);
  await seedLegacy(page);
  await page.goto("./?shared=%2F%2Fattacker.example");
  await expect(page.locator("#load-error")).toBeVisible();
  await expect(page.locator("#load-error")).toContainText("Share it again");
  await expect.poll(() => page.evaluate(async () => (await (await caches.open("hexscope-shared")).match("shared-file")) !== undefined)).toBe(true);
});

test("an incomplete submission is discarded instead of opening a partial batch", async ({ page }) => {
  await controlledHome(page);
  const id = "00000000-0000-4000-8000-000000000013";
  await seedIncompleteSubmission(page, id);
  await page.goto(`./?shared=${id}`);
  await expect(page.locator("#load-error")).toBeVisible();
  await expect(page.locator("#load-error")).toContainText("Share it again");
  await expect(page.locator(".verdict-title")).toHaveCount(0);
  await expect.poll(() => page.evaluate(async (submission) => (await (await caches.open("hexscope-shared")).match(new URL(`shared/${submission}/0`, location.href))) === undefined, id)).toBe(true);
});

test("share errors use the selected language and remove their query string", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  await page.goto("./?share-error=size");
  await expect(page.locator("#load-error")).toBeVisible();
  await expect(page.locator("#load-error")).toContainText("Набір завеликий");
  await expect(page).not.toHaveURL(/share-error=/);
});
