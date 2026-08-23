/*
 * The entry surface's own geometry. Its hero sits a share of the viewport down, and
 * the share is of the viewport a reader can see, which only a retracting toolbar
 * tells apart from the largest one - no headless frame has one. So the share is
 * read off the box and the unit off the cascade.
 */

import { expect, test } from "@playwright/test";

import { mountApi } from "./fixtures.js";

/** The share of the viewport the hero is held down by, as `entry.css` declares it. */
const HERO_SHARE = 0.12;

test.describe("the entry surface", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, { notes: [] });
    await page.goto("/");
    await expect(page.getByRole("combobox", { name: "Search notes" })).toBeVisible();
  });

  test("holds its hero a share of the visible viewport down", async ({ page }) => {
    const hero = await page.evaluate(() => {
      const surface = document.querySelector(".entry") as HTMLElement;
      const authored = Array.from(document.styleSheets)
        .flatMap((sheet) => Array.from(sheet.cssRules))
        .filter(
          (rule): rule is CSSStyleRule =>
            rule instanceof CSSStyleRule && rule.selectorText === ".entry",
        );
      return {
        drawn: Number.parseFloat(getComputedStyle(surface).paddingTop),
        visibleViewport: window.visualViewport?.height ?? 0,
        // The rule as the engine kept it, which holds the dynamic declaration
        // wherever the unit parses and the `vh` line beneath it wherever it does not.
        rule: authored.at(-1)?.cssText ?? "",
      };
    });

    expect(Math.round(hero.drawn)).toBe(
      Math.round(HERO_SHARE * hero.visibleViewport),
    );
    expect(hero.rule).toContain(`${HERO_SHARE * 100}dvh`);
  });
});
