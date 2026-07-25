/// <reference types="node" />
import { defineConfig, devices } from "@playwright/test";

/*
 * End-to-end configuration for the reading client.
 *
 * Specs run against the built client served by `vite preview`, so `dist/` must
 * exist before the preview server starts; the `test-e2e` Makefile target
 * depends on `build-web`.
 */
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  use: {
    baseURL: "http://localhost:4173",
    // A fixed desktop frame the spine-geometry assertions measure against; the
    // mobile spec overrides it per file.
    viewport: { width: 1200, height: 900 },
    // Reduced motion makes the spine's scripted reveal scroll jump to its final
    // offset, so scroll-position assertions see a settled value.
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
    command: "npm run preview -- --port 4173 --strictPort",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
