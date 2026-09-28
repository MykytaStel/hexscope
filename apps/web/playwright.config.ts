// End-to-end: the built site, served as a static host would, in a real
// browser on a computer's screen and a phone's. `pnpm build` first.
import { defineConfig, devices } from "@playwright/test";

const PORT = 4175;

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
    { name: "computer", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } },
    { name: "phone", use: { ...devices["Pixel 7"] } },
  ],
  webServer: {
    command: `pnpm preview --port ${PORT} --strictPort`,
    port: PORT,
    reuseExistingServer: !process.env.CI,
  },
});
