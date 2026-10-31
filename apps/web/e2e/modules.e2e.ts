// The parser is built per family of files: a file loads the build that
// reads it and none of the others.
import { expect, test, type Page } from "@playwright/test";
import { Buffer } from "node:buffer";

async function wasmLoadedFor(page: Page, sample: string): Promise<string[]> {
  const loaded: string[] = [];
  page.on("request", (r) => {
    const name = /\/(hexscope_[a-z]+)_bg-[^/]+\.wasm$/.exec(r.url())?.[1];
    if (name) loaded.push(name);
  });
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto(`./?sample=${sample}`);
  await expect(page.locator(".verdict-title")).toBeVisible();
  return [...new Set(loaded)].sort();
}

function mp3(frameId: "TIT2" | "TXXX"): Buffer {
  const payload = frameId === "TIT2" ? Buffer.from([3, ...Buffer.from("song")]) : Buffer.from([3, ...Buffer.from("custom\0value")]);
  const body = Buffer.concat([
    Buffer.from(frameId),
    Buffer.from([0, 0, 0, payload.length, 0, 0]),
    payload,
  ]);
  return Buffer.concat([
    Buffer.from([0x49, 0x44, 0x33, 4, 0, 0, 0, 0, 0, body.length]),
    body,
    Buffer.from([0xff, 0xfb, 0x90, 0x64, 1, 2, 3, 4]),
  ]);
}

function flac(
  withUninspectedBlock = false,
  withFrames = true,
  withPicture = false,
  withComments = true,
): Buffer {
  const vendor = Buffer.from("test encoder");
  const title = Buffer.from("TITLE=song");
  const comments = Buffer.concat([
    Buffer.from([vendor.length, 0, 0, 0]),
    vendor,
    Buffer.from([1, 0, 0, 0]),
    Buffer.from([title.length, 0, 0, 0]),
    title,
  ]);
  const streamInfo = Buffer.concat([Buffer.from([0, 0, 0, 34]), Buffer.alloc(34)]);
  const unknown = withUninspectedBlock
    ? Buffer.from([2, 0, 0, 1, 0x41])
    : Buffer.alloc(0);
  const commentBlock = withComments
    ? Buffer.concat([
      Buffer.from([withPicture ? 0x04 : 0x84, (comments.length >> 16) & 0xff, (comments.length >> 8) & 0xff, comments.length & 0xff]),
      comments,
    ])
    : Buffer.alloc(0);
  const pictureBody = Buffer.from([0, 0, 0, 3, 0x50, 0x4e, 0x47, 0, 0, 0, 0, 1, 2, 3]);
  const pictureBlock = withPicture
    ? Buffer.concat([
      Buffer.from([0x86, (pictureBody.length >> 16) & 0xff, (pictureBody.length >> 8) & 0xff, pictureBody.length & 0xff]),
      pictureBody,
    ])
    : Buffer.alloc(0);
  return Buffer.concat([
    Buffer.from("fLaC"),
    streamInfo,
    unknown,
    commentBlock,
    pictureBlock,
    ...(withFrames ? [Buffer.from([0xff, 0xf8, 0x69, 0x08, 1, 2, 3, 4])] : []),
  ]);
}

for (const [sample, build] of [
  ["photo.jpg", "hexscope_media"],
  ["drawing.svg", "hexscope_media"],
  ["report.pdf", "hexscope_pdf"],
  ["report.docx", "hexscope_docs"],
  ["message.eml", "hexscope_docs"],
] as const) {
  test(`${sample} loads ${build} alone`, async ({ page }) => {
    expect(await wasmLoadedFor(page, sample)).toEqual([build]);
  });
}

test("an MP3 file loads its parser, links the tag to its bytes, and confirms the clean copy", async ({ page }) => {
  const loaded: string[] = [];
  page.on("request", (r) => {
    const name = /\/(hexscope_[a-z]+)_bg-[^/]+\.wasm$/.exec(r.url())?.[1];
    if (name) loaded.push(name);
  });
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.locator("#picker-empty").setInputFiles({
    name: "song.mp3",
    mimeType: "audio/mpeg",
    buffer: mp3("TIT2"),
  });

  await expect(page.locator(".verdict-title")).toBeVisible();
  await expect(page.getByRole("heading", { name: "What this audio file reveals" })).toBeVisible();
  await page.getByRole("button", { name: "Metadata", exact: true }).click();
  await expect(page.getByRole("button", { name: "song" })).toBeVisible();
  await page.getByRole("button", { name: "Overview", exact: true }).click();
  await expect(page.locator(".verdict-cta")).toBeVisible();
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-what")).toHaveText("confirmed removed from the copy");
  await page.locator(".clean-removed > summary").click();
  await expect(page.locator(".clean-list")).toContainText("ID3v2 metadata tags");
  // A service worker may pre-cache the media build during this page visit;
  // the audio parser itself must still be requested for the uploaded file.
  expect(loaded).toContain("hexscope_audio");
});

test("a FLAC keeps its audio frames and confirms removal of the comment block", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.locator("#picker-empty").setInputFiles({
    name: "song.flac",
    mimeType: "audio/flac",
    buffer: flac(),
  });

  await expect(page.locator(".verdict-title")).toBeVisible();
  await expect(page.locator(".cleaner .clean-copy-note")).toContainText("Removes FLAC Vorbis comments and embedded pictures such as cover art.");
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-what")).toContainText("confirmed removed from the copy");
  await page.locator(".clean-removed > summary").click();
  await expect(page.locator(".clean-list")).toContainText("FLAC Vorbis comments");
});

test("a FLAC cover-art-only file can make a copy without its picture block", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.locator("#picker-empty").setInputFiles({
    name: "cover-art-only.flac",
    mimeType: "audio/flac",
    buffer: flac(false, true, true, false),
  });

  await expect(page.locator(".verdict-title")).toBeVisible();
  await expect(page.locator(".verdict-cta")).toBeVisible();
  await page.locator(".verdict-cta").click();
  await page.locator(".clean-removed > summary").click();
  await expect(page.locator(".clean-list")).toContainText("FLAC pictures");
});

test("a FLAC with an uninspected metadata block does not offer a clean copy", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.locator("#picker-empty").setInputFiles({
    name: "partial.flac",
    mimeType: "audio/flac",
    buffer: flac(true),
  });

  await expect(page.getByText("Some audio metadata could not be fully read; the result is partial.")).toBeVisible();
  await expect(page.locator(".verdict-cta")).toHaveCount(0);
  await expect(page.locator(".reveals").getByText("Read-only metadata inspection. Audio playback and clean copies are not provided.")).toBeVisible();
});

test("a tagged FLAC without audio frames does not offer a clean copy", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.locator("#picker-empty").setInputFiles({
    name: "missing-frames.flac",
    mimeType: "audio/flac",
    buffer: flac(false, false),
  });

  await expect(page.locator(".verdict-title")).toBeVisible();
  await expect(page.locator(".verdict-cta")).toHaveCount(0);
});

test("an MP3 tag without an audio frame does not offer a clean copy", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.locator("#picker-empty").setInputFiles({
    name: "untagged.mp3",
    mimeType: "audio/mpeg",
    buffer: Buffer.from([0x49, 0x44, 0x33, 4, 0, 0, 0, 0, 0, 0]),
  });

  await expect(page.locator(".verdict-title")).toContainText("No supported audio metadata tags were found.");
  await expect(page.locator(".reveals").getByText("No supported audio metadata tags were found.")).toBeVisible();
  await expect(page.locator(".reveals").getByText("Read-only metadata inspection. Audio playback and clean copies are not provided.")).toBeVisible();
  await expect(page.getByRole("button", { name: /clean copy/i })).toHaveCount(0);
  await expect(page.locator("audio")).toHaveCount(0);
});

test("an unsupported ID3 frame can be removed but its findings stay unchecked", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.locator("#picker-empty").setInputFiles({
    name: "partial.mp3",
    mimeType: "audio/mpeg",
    buffer: mp3("TXXX"),
  });

  await expect(page.locator(".verdict-title")).toContainText("could not be fully read");
  await expect(page.getByRole("heading", { name: "What this audio file reveals" })).toBeVisible();
  await expect(page.getByText("Some audio metadata could not be fully read; the result is partial.")).toBeVisible();
  const partialFinding = page.locator(".verdict-line.is-warning").filter({
    hasText: "Some audio metadata could not be fully read; this is a partial inspection.",
  });
  const findInFile = partialFinding.getByRole("button", { name: "Find in file" });
  await expect(findInFile).toBeVisible();
  await findInFile.click();
  await expect(page.locator("#statusbar-file")).toContainText("ID3 frame TXXX not inspected");
  await page.getByRole("button", { name: "Overview", exact: true }).click();
  await page.locator(".verdict-cta").click();
  await expect(page.locator(".ba-side.is-after .ba-what")).toHaveText("Copy not fully checked");
});
