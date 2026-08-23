
import { expect, test, type Locator, type Page } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:origin.org",
      title: "Origin Note",
      body: "A note the reader reaches through the field.",
    },
  ],
};

interface Ring {
  readonly width: number;
  readonly style: string;
  readonly color: string;
  readonly borderColor: string;
}

function ringOf(target: Locator): Promise<Ring> {
  return target.evaluate((node) => {
    const style = getComputedStyle(node);
    return {
      width: Number.parseFloat(style.outlineWidth),
      style: style.outlineStyle,
      color: style.outlineColor,
      borderColor: style.borderTopColor,
    };
  });
}

function channels(color: string): number[] {
  return [...color.matchAll(/\d+/g)].slice(0, 3).map((match) => Number(match[0]));
}

function lightness(color: string): number {
  const [r = 0, g = 0, b = 0] = channels(color);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function canvasColor(page: Page): Promise<string> {
  return page.evaluate(() => getComputedStyle(document.body).backgroundColor);
}

async function blurField(page: Page): Promise<void> {
  await page.locator(".entry-hero__root").click();
  await expect(page.locator("body")).toBeFocused();
}

const field = (page: Page): Locator =>
  page.getByRole("combobox", { name: "Search the slipbox" });

for (const scheme of ["light", "dark"] as const) {
  test.describe(`search field focus in the ${scheme} scheme`, () => {
    test.use({ colorScheme: scheme });

    test.beforeEach(async ({ page }) => {
      await mountApi(page, WORLD);
      await page.goto("/");
      await expect(field(page)).toBeVisible();
    });

    test("draws a ring the canvas cannot hide, and moves no layout", async ({
      page,
    }) => {
      const target = field(page);
      await blurField(page);
      const resting = await ringOf(target);
      const restingBox = await target.boundingBox();
      expect(resting.style).toBe("none");

      await page.keyboard.press("Shift+Tab");
      await page.keyboard.press("Tab");
      await expect(target).toBeFocused();

      const focused = await ringOf(target);
      expect(focused.style).toBe("solid");
      expect(focused.width).toBeGreaterThanOrEqual(2);
      const canvas = lightness(await canvasColor(page));
      expect(Math.abs(lightness(focused.color) - canvas)).toBeGreaterThan(60);

      expect(focused.borderColor).not.toBe(resting.borderColor);
      expect(await target.boundingBox()).toEqual(restingBox);
    });

    test("draws that ring on the glossary's own field too", async ({ page }) => {
      await page.getByRole("button", { name: "Glossary" }).click();
      const target = page.getByRole("combobox", { name: "Search the glossary" });
      await expect(target).toBeVisible();

      await page.getByRole("button", { name: "All terms" }).click();
      const resting = await ringOf(target);
      const restingBox = await target.boundingBox();
      expect(resting.style).toBe("none");

      await page.keyboard.press("Tab");
      await page.keyboard.press("Tab");
      await expect(target).toBeFocused();

      const focused = await ringOf(target);
      expect(focused.style).toBe("solid");
      expect(focused.width).toBeGreaterThanOrEqual(2);
      const canvas = lightness(await canvasColor(page));
      expect(Math.abs(lightness(focused.color) - canvas)).toBeGreaterThan(60);

      expect(focused.borderColor).not.toBe(resting.borderColor);
      expect(await target.boundingBox()).toEqual(restingBox);
    });
  });
}

test.describe("focus elsewhere on the entry surface", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
    await page.goto("/");
    await expect(field(page)).toBeVisible();
  });

  test("keeps the platform ring on a control that never suppressed it", async ({
    page,
  }) => {
    const random = page.getByRole("button", { name: "Surprise me" });
    await page.keyboard.press("Tab");
    await expect(random).toBeFocused();
    expect((await ringOf(random)).style).not.toBe("none");
  });

  test("paints a clicked field, which the platform counts as visibly focused", async ({
    page,
  }) => {
    const target = field(page);
    await blurField(page);
    await target.click();
    await expect(target).toBeFocused();
    expect((await ringOf(target)).style).toBe("solid");
  });
});
