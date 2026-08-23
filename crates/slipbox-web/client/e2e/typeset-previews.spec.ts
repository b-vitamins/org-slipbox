/*
 * KaTeX only typesets where it can measure fonts, so these assertions need a
 * real browser rather than jsdom.
 */

import { expect, test, type Locator } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:flows.org",
      title: "Normalizing flows",
      body: "Built on the [[id:change-uuid][change of variables]] identity.",
      forwardLinks: [
        {
          key: "file:change.org",
          id: "change-uuid",
          title: "Change of variables",
          preview: "Built on the change of variables identity.",
        },
      ],
      // A row's preview quotes the other note, so the math a preview has to
      // typeset rides on the backlink rather than on the forward link.
      backlinks: [
        {
          key: "file:change.org",
          id: "change-uuid",
          title: "Change of variables",
          preview: "bounded by \\(\\sum_n x_n\\) throughout",
        },
      ],
    },
    {
      key: "file:change.org",
      id: "change-uuid",
      title: "Change of variables",
      body: "The density of \\(\\mathbf{x}\\) transforms by the Jacobian.",
    },
  ],
};

/**
 * KaTeX renders every formula twice: typeset glyphs plus a `.katex-mathml`
 * annotation holding the original TeX. Only the glyphs are on screen, so the
 * annotation is dropped before reading `textContent`.
 */
async function visibleText(locator: Locator): Promise<string> {
  return locator.evaluate((element) => {
    const visible = element.cloneNode(true) as Element;
    for (const mathml of visible.querySelectorAll(".katex-mathml")) {
      mathml.remove();
    }
    return (visible.textContent ?? "").replace(/\s+/g, " ").trim();
  });
}

test.describe("typeset previews", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
    await page.goto("/?note=file:flows.org");
    await expect(page.getByRole("heading", { name: "Normalizing flows" })).toBeVisible();
  });

  test("a glance card typesets the math in its excerpt", async ({ page }) => {
    await page.locator(".spine-column").last().locator("a.org-link").first().hover();

    const excerpt = page.locator(".glance-card__excerpt");
    await expect(excerpt).toBeVisible();
    await expect(excerpt.locator(".katex")).toHaveCount(1);
    expect(await visibleText(excerpt)).toBe("The density of x transforms by the Jacobian.");
  });

  test("a relation row typesets the math in its linking line", async ({ page }) => {
    const preview = page.locator(".relations__preview").first();
    await expect(preview).toBeVisible();
    await expect(preview.locator(".katex")).toHaveCount(1);
    expect(await visibleText(preview)).toBe("bounded by ∑n​xn​ throughout");
  });

  // A search excerpt is a slice of note source, so it is the one preview that
  // arrives with Org markup still in it.
  test("a search excerpt typesets its math and unwraps its links", async ({ page }) => {
    await page.goto("/");
    await page.getByPlaceholder("Search notes").fill("transforms");

    const excerpt = page.locator(".entry-result__snippet").first();
    await expect(excerpt).toBeVisible();
    await expect(excerpt.locator(".katex")).toHaveCount(1);
    expect(await visibleText(excerpt)).toBe("The density of x transforms by the Jacobian.");
    expect(await visibleText(excerpt.locator("mark"))).toBe("transforms");
  });

  test("a search excerpt shows a link's words rather than its id", async ({ page }) => {
    await page.goto("/");
    await page.getByPlaceholder("Search notes").fill("identity");

    const excerpt = page.locator(".entry-result__snippet").first();
    await expect(excerpt).toBeVisible();
    expect(await visibleText(excerpt)).toBe("change of variables identity.");
    await expect(excerpt.locator("a")).toHaveCount(0);
  });

  // KaTeX's own default is 1.21em, so the stylesheet overrides it back to the
  // surrounding row's size.
  test("preview math is set at the size of the prose around it", async ({ page }) => {
    const preview = page.locator(".relations__preview").first();
    const rowSize = await preview.evaluate((el) => getComputedStyle(el).fontSize);
    const mathSize = await preview
      .locator(".katex")
      .evaluate((el) => getComputedStyle(el).fontSize);

    expect(mathSize).toBe(rowSize);
  });
});
