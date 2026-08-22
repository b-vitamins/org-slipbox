
import { expect, test } from "@playwright/test";

import { mountApi } from "./fixtures.js";

test.describe("document shell", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, { notes: [] });
  });

  test("declares a tab icon the build serves as an image", async ({ page }) => {
    await page.goto("/");
    const href = await page.locator("link[rel=icon]").getAttribute("href");
    expect(href).toBe("./favicon.svg");

    const icon = await page.request.get(new URL(href as string, page.url()).href);
    expect(icon.status()).toBe(200);
    expect(icon.headers()["content-type"]).toContain("image/svg+xml");

    const size = await page.evaluate(
      (src) =>
        new Promise<number>((resolve) => {
          const probe = new Image();
          probe.addEventListener("load", () => resolve(probe.naturalWidth));
          probe.addEventListener("error", () => resolve(0));
          probe.src = src;
        }),
      href as string,
    );
    expect(size).toBeGreaterThan(0);
  });

  test("the icon inverts for a dark tab strip", async ({ page }) => {
    const icon = await (await page.request.get("/favicon.svg")).text();
    expect(icon).toContain("prefers-color-scheme: dark");
  });

  test("a load asks for no path the server does not have", async ({ page }) => {
    const missing: string[] = [];
    page.on("response", (response) => {
      const path = new URL(response.url()).pathname;
      if (response.status() === 404 && !path.startsWith("/api/")) {
        missing.push(path);
      }
    });

    await page.goto("/");
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();

    expect(missing).toEqual([]);
  });
});
