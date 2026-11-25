import { defineConfig } from "@playwright/test";

const port = Number(process.env.HEXSCOPE_PERF_PORT ?? 4185);

// Separate from the E2E shard matrix: no other benchmark workers contend
// for CPU and no automatic retries can hide a slow or failed observation.
export default defineConfig({
  testDir: "perf",
  testMatch: "*.perf.ts",
  workers: 1,
  fullyParallel: false,
  retries: 0,
  timeout: 600_000,
  reporter: "list",
  use: { browserName: "chromium", headless: true },
  webServer: {
    command: `pnpm preview --port ${port} --strictPort`,
    port,
    reuseExistingServer: false,
  },
  metadata: { baseURL: `http://localhost:${port}/` },
});
