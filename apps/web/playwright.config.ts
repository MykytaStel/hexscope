// End-to-end: the built site, served as a static host would, in a real
// browser on a computer's screen and a phone's. `pnpm build` first.
import { defineConfig, devices } from "@playwright/test";

// Never silently test a different checkout's preview server on the same port.
// Pick another port when a local server is already running.
const PORT = Number(process.env.HEXSCOPE_TEST_PORT ?? 4175);
const CROSS_BROWSER_SPEC = /cross-browser\.e2e\.ts/;

export default defineConfig({
  testDir: "e2e",
  testMatch: "*.e2e.ts",
  timeout: 60_000,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  use: {
    baseURL: `http://localhost:${PORT}/`,
    // Every download is caught by the test, never written for keeps.
    acceptDownloads: true,
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "computer",
      testIgnore: CROSS_BROWSER_SPEC,
      use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } },
    },
    { name: "phone", testIgnore: CROSS_BROWSER_SPEC, use: { ...devices["Pixel 7"] } },
    {
      name: "desktop-firefox",
      testMatch: CROSS_BROWSER_SPEC,
      use: { browserName: "firefox", viewport: { width: 1280, height: 800 } },
    },
    {
      name: "desktop-webkit",
      testMatch: CROSS_BROWSER_SPEC,
      use: { browserName: "webkit", viewport: { width: 1280, height: 800 } },
    },
  ],
  webServer: {
    command: `pnpm preview --port ${PORT} --strictPort`,
    port: PORT,
    reuseExistingServer: false,
  },
});
