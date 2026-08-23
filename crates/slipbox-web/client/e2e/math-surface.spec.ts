/*
 * A display equation wide enough to scroll covers its own edges with a gradient
 * in the surface behind it, so what that surface is has to be read off the
 * cascade: the same renderer sets an equation in the reading column and in a
 * glossary definition, which stand on different tones. Only a browser has a
 * cascade and a painted gradient to read back.
 */

import { expect, test, type Locator } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

/** Wider than either surface, so the covers have an edge to hide. */
const EQUATION =
  "\\[ \\int_0^1 x^2 \\, dx + \\sum_{i=1}^{n} a_i b_i + " +
  "\\prod_{j=1}^{m} c_j = \\frac{1}{3} + \\alpha + \\beta + \\gamma \\]";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:reading.org",
      title: "Reading Note",
      body: `An equation stands here.\n\n${EQUATION}`,
    },
    {
      key: "file:term.org",
      title: "Term",
      body: `A definition, and an equation.\n\n${EQUATION}`,
      glossaryStatus: "confirmed",
    },
  ],
};

/** The two colors one cover gradient runs between, the opaque end first. */
function cover(math: Locator): Promise<{ opaque: string; faded: string }> {
  return math.evaluate((node) => {
    const painted = getComputedStyle(node).backgroundImage;
    const colors = painted.match(/rgba?\([^)]*\)/g) ?? [];
    return { opaque: colors[0] ?? painted, faded: colors[1] ?? painted };
  });
}

/** The tone painted behind an element, from the nearest box that paints one. */
function behind(math: Locator): Promise<string> {
  return math.evaluate((node) => {
    for (let box = node.parentElement; box; box = box.parentElement) {
      const painted = getComputedStyle(box).backgroundColor;
      if (painted !== "rgba(0, 0, 0, 0)") {
        return painted;
      }
    }
    return "";
  });
}

/** The same channels at zero alpha, spelled as a browser prints them. */
function zeroAlpha(color: string): string {
  return color.replace("rgb(", "rgba(").replace(")", ", 0)");
}

test.describe("a math scroll shadow", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  for (const scheme of ["light", "dark"] as const) {
    test.describe(`in the ${scheme} scheme`, () => {
      test.use({ colorScheme: scheme });

      test("covers each edge in the surface that equation stands on", async ({
        page,
      }) => {
        await page.goto("/?note=file:reading.org");
        const inColumn = page.locator(".org-math--display").first();
        await expect(inColumn).toBeVisible();
        const column = await cover(inColumn);
        expect(column.opaque).toBe(await behind(inColumn));

        await page.goto("/?view=glossary");
        const inPane = page.locator(".org-math--display").first();
        await expect(inPane).toBeVisible();
        const pane = await cover(inPane);
        expect(pane.opaque).toBe(await behind(inPane));

        // The slab this replaces: the pane's own tone is not the column's, so a
        // cover taken from one of them is visible on the other.
        expect(pane.opaque).not.toBe(column.opaque);

        // Each cover runs out through its own hue rather than through the
        // transparent black `transparent` would fade to.
        expect(column.faded).toBe(zeroAlpha(column.opaque));
        expect(pane.faded).toBe(zeroAlpha(pane.opaque));
      });
    });
  }
});
