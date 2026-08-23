
import { expect, test } from "@playwright/test";

import { mountApi } from "./fixtures.js";

const HERO_SHARE = 0.12;

test.describe("the entry surface", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, { notes: [] });
    await page.goto("/");
    await expect(page.getByRole("combobox", { name: "Search the slipbox" })).toBeVisible();
  });

  test("asks for a search without narrowing the corpus", async ({ page }) => {
    await expect(page.getByRole("combobox", { name: "Search the slipbox" })).toHaveAttribute(
      "placeholder",
      "What are you looking for?",
    );
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
        rule: authored.at(-1)?.cssText ?? "",
      };
    });

    expect(Math.round(hero.drawn)).toBe(
      Math.round(HERO_SHARE * hero.visibleViewport),
    );
    expect(hero.rule).toContain(`${HERO_SHARE * 100}dvh`);
  });
});
