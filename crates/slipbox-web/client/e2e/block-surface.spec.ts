
import { expect, test, type Locator } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

const EQUATION =
  "\\[ \\int_0^1 x^2 \\, dx + \\sum_{i=1}^{n} a_i b_i + " +
  "\\prod_{j=1}^{m} c_j = \\frac{1}{3} + \\alpha + \\beta + \\gamma \\]";

const CODE = "#+begin_src python\nprint(1)\n#+end_src\n\nAnd =verbatim= inline.";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:reading.org",
      title: "Reading Note",
      body: `An equation stands here.\n\n${EQUATION}\n\n${CODE}`,
    },
    {
      key: "file:term.org",
      title: "Term",
      body: `A definition, and an equation.\n\n${EQUATION}\n\n${CODE}`,
      glossaryStatus: "confirmed",
    },
  ],
};

function cover(math: Locator): Promise<{ opaque: string; faded: string }> {
  return math.evaluate((node) => {
    const painted = getComputedStyle(node).backgroundImage;
    const colors = painted.match(/rgba?\([^)]*\)/g) ?? [];
    return { opaque: colors[0] ?? painted, faded: colors[1] ?? painted };
  });
}

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

function painted(box: Locator): Promise<string> {
  return box.evaluate((node) => getComputedStyle(node).backgroundColor);
}

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

        expect(pane.opaque).not.toBe(column.opaque);

        expect(column.faded).toBe(zeroAlpha(column.opaque));
        expect(pane.faded).toBe(zeroAlpha(pane.opaque));
      });
    });
  }
});

test.describe("a code block", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  for (const scheme of ["light", "dark"] as const) {
    test.describe(`in the ${scheme} scheme`, () => {
      test.use({ colorScheme: scheme });

      test("paints a tone the surface it stands on does not", async ({ page }) => {
        for (const [where, address] of [
          ["column", "/?note=file:reading.org"],
          ["pane", "/?view=glossary"],
        ] as const) {
          await page.goto(address);
          for (const selector of [".org-src", ".org-verbatim"]) {
            const block = page.locator(selector).first();
            await expect(block).toBeVisible();
            expect(await painted(block), `${selector} in the ${where}`).not.toBe(
              await behind(block),
            );
          }

          const copy = page.locator(".org-src__copy").first();
          await copy.hover();
          expect(await painted(copy), `the copy control in the ${where}`).not.toBe(
            await painted(page.locator(".org-src").first()),
          );
        }
      });
    });
  }
});
