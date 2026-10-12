// Records the README's demo: the built site, a sample photo opened, what it
// gives away, and its clean copy checked. Needs `pnpm build` and ffmpeg.
//
//   node scripts/make-demo.mjs   → docs/demo.gif
import { execFileSync, spawn } from "node:child_process";
import { mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createRequire } from "node:module";

const root = new URL("..", import.meta.url).pathname;
const web = join(root, "apps/web");
const { chromium } = createRequire(join(web, "package.json"))("@playwright/test");
const port = 4179;
const server = spawn("pnpm", ["exec", "vite", "preview", "--port", String(port), "--strictPort"], { cwd: web, stdio: "ignore" });
const videos = mkdtempSync(join(tmpdir(), "hexscope-demo-"));
const pause = (ms) => new Promise((r) => setTimeout(r, ms));

try {
  await pause(2500);
  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    colorScheme: "dark",
    acceptDownloads: true,
    recordVideo: { dir: videos, size: { width: 1280, height: 800 } },
  });
  await context.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  const page = await context.newPage();
  await page.goto(`http://localhost:${port}/`);
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await pause(1800);
  await page.getByRole("button", { name: /sample photo/i }).first().click();
  await page.locator(".verdict-title").waitFor();
  await pause(2600);
  await page.locator(".verdict-show").first().click();
  await pause(1800);
  await page.locator(".verdict-cta").click();
  await page.locator(".before-after").waitFor();
  await pause(3200);
  await context.close();
  await browser.close();
  const [video] = readdirSync(videos).filter((f) => f.endsWith(".webm"));
  execFileSync("ffmpeg", [
    "-loglevel", "error", "-y", "-i", join(videos, video),
    "-vf", "fps=8,scale=800:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=48:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
    join(root, "docs/demo.gif"),
  ]);
  console.log("docs/demo.gif");
} finally {
  server.kill();
  rmSync(videos, { recursive: true, force: true });
}
