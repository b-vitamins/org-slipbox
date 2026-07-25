/*
 * Every color token is a `light-dark()` pair resolved against `color-scheme`.
 * jsdom has no cascade and no computed color, so a real browser is the only
 * place the painted canvas can be read. The `colorScheme` context option
 * emulates the platform preference these specs assert against.
 */

import { expect, test, type Page } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:origin.org",
      title: "Origin Note",
      body: "A note whose canvas the reader repaints, over \\(x^2\\) as well.",
    },
  ],
};

/** The painted background of the reading canvas, as `rgb(...)`. */
function paperColor(page: Page): Promise<string> {
  return page.evaluate(() => getComputedStyle(document.body).backgroundColor);
}

/** The painted body text color, as `rgb(...)`. */
function inkColor(page: Page): Promise<string> {
  return page.evaluate(() => getComputedStyle(document.body).color);
}

/** Parse an `rgb(r, g, b)` string into its channels. */
function channels(color: string): number[] {
  return [...color.matchAll(/\d+/g)].slice(0, 3).map((match) => Number(match[0]));
}

/** Perceived lightness of a painted color, 0 (black) to 255 (white). */
function lightness(color: string): number {
  const [r = 0, g = 0, b = 0] = channels(color);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

test.describe("color scheme", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test.describe("following the platform", () => {
    test.use({ colorScheme: "dark" });

    test("paints the dark canvas with no stored choice", async ({ page }) => {
      await page.goto("/?note=file:origin.org");
      await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

      await expect(page.locator("html")).not.toHaveAttribute("data-theme", /.*/);
      await expect(page.getByRole("button", { name: "Color scheme: Auto" })).toBeVisible();

      const paper = await paperColor(page);
      const ink = await inkColor(page);
      expect(lightness(paper)).toBeLessThan(64);
      expect(lightness(ink)).toBeGreaterThan(160);
    });

    test("a reader can override the platform back to light", async ({ page }) => {
      await page.goto("/?note=file:origin.org");
      await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();
      const control = page.getByRole("button", { name: /^Color scheme:/ });

      // The control cycles Auto -> Light -> Dark -> Auto.
      await control.click();
      await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
      expect(lightness(await paperColor(page))).toBeGreaterThan(200);

      await control.click();
      await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
      expect(lightness(await paperColor(page))).toBeLessThan(64);

      await control.click();
      await expect(page.locator("html")).not.toHaveAttribute("data-theme", /.*/);
      expect(lightness(await paperColor(page))).toBeLessThan(64);
    });
  });

  test.describe("overriding the platform", () => {
    test.use({ colorScheme: "light" });

    test("a stored dark choice paints dark on first load, with no light flash", async ({
      page,
    }) => {
      await page.goto("/?note=file:origin.org");
      await page.getByRole("button", { name: /^Color scheme:/ }).click();
      await page.getByRole("button", { name: /^Color scheme:/ }).click();
      await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

      // Holding the module bundle in flight across a reload leaves the inline
      // script in index.html as the only code that has run, so the attribute
      // read below is the head's work rather than the store's.
      let releaseBundle = (): void => {};
      const held = new Promise<void>((resolve) => {
        releaseBundle = resolve;
      });
      await page.route("**/assets/*.js", async (route) => {
        await held;
        await route.continue();
      });

      await page.reload({ waitUntil: "commit" });
      await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
      expect(lightness(await paperColor(page))).toBeLessThan(64);

      releaseBundle();
      await page.unroute("**/assets/*.js");
      await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();
      expect(lightness(await paperColor(page))).toBeLessThan(64);
      await expect(page.getByRole("button", { name: "Color scheme: Dark" })).toBeVisible();
    });

    test("typeset math inherits the scheme's ink rather than a baked color", async ({
      page,
    }) => {
      await page.goto("/?note=file:origin.org");
      const math = page.locator(".org-math--inline .katex").first();
      await expect(math).toBeVisible();
      const lightMath = await math.evaluate((node) => getComputedStyle(node).color);

      await page.getByRole("button", { name: /^Color scheme:/ }).click();
      await page.getByRole("button", { name: /^Color scheme:/ }).click();
      await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

      // KaTeX sets no color of its own, so the formula inherits the scheme's ink.
      const darkMath = await math.evaluate((node) => getComputedStyle(node).color);
      expect(darkMath).not.toBe(lightMath);
      expect(lightness(darkMath)).toBeGreaterThan(160);
    });
  });
});
