/// <reference types="node" />
import { defineConfig, devices } from "@playwright/test";

/*
 * End-to-end configuration for the standalone document bundle. `dist-document/`
 * must exist before the preview server starts, and is served as a plain static
 * directory with no application and no API proxy. The server is owned by this
 * run, on a port of its own, so an already-occupied 4174 is a failure rather than
 * a server to adopt.
 */
export default defineConfig({
  testDir: "./e2e-document",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  use: {
    baseURL: "http://localhost:4174",
    viewport: { width: 1200, height: 900 },
    contextOptions: { reducedMotion: "reduce" },
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1200, height: 900 } },
    },
  ],
  webServer: {
    command: "npm run preview:document -- --port 4174 --strictPort",
    url: "http://localhost:4174/host.html",
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
