/*
 * The glossary dictionary's peek pane as a navigable surface.
 *
 * The peek renders a definition with the same renderer the reading column uses,
 * so its links are real anchors; what a real browser adds over jsdom is the
 * hoverless pointer, whose tap takes a hover's place in the grammar and so has
 * to be honored on a surface carrying no preview card.
 */

import { expect, test } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:entropy.org",
      title: "Entropy",
      body: "A measure of uncertainty, dual to [[id:prior-uuid][the prior]].",
      glossaryStatus: "confirmed",
    },
    {
      key: "file:prior.org",
      id: "prior-uuid",
      title: "Prior",
      body: "What the model believes before it sees the data.",
    },
  ],
};

/** The definition the sole marked term peeks on arrival. */
const DEFINITION = "A measure of uncertainty, dual to";

/** Terms the fixture serves per page, and a world holding more than one page. */
const PAGE = 25;
const LONG: FixtureWorld = {
  notes: Array.from({ length: 30 }, (_, index) => ({
    key: `file:term-${index + 1}.org`,
    title: `Term ${index + 1}`,
    body: `Definition ${index + 1}.`,
    glossaryStatus: "confirmed" as const,
  })),
};

/** The `min-height` the stylesheet gives a control under a coarse pointer. */
const TOUCH_TARGET = 44;

test.describe("the glossary peek", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("a link inside a definition opens the note it names", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    await page.getByRole("link", { name: "the prior" }).click();

    // The glossary is an entry surface, so opening a note replaces it outright
    // rather than stacking a column beside the definition.
    await expect(page.getByRole("heading", { name: "Prior" })).toBeVisible();
    await expect(page.getByText("What the model believes")).toBeVisible();
    await expect(page.locator(".glossary")).toHaveCount(0);
  });

  test("resting on a link in a definition commits nothing", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    await page.getByRole("link", { name: "the prior" }).hover();

    // Opening replaces this surface synchronously with the hover that asked for
    // it, so a glossary still mounted after the hover resolves is one that
    // committed to nothing. No card either: the preview is the spine's chrome.
    await expect(page.locator(".glossary")).toBeVisible();
    await expect(page.locator(".glance-card")).toHaveCount(0);
  });

  test("the term's own control opens it in the reader", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    const control = page.getByRole("link", { name: "Open in reader" });
    expect(await control.getAttribute("href")).toBe("?note=file%3Aentropy.org");

    await control.click();

    // The peek heads its definition with an h2; the reading column heads a note
    // with an h1, so the level is what separates the two surfaces here.
    await expect(
      page.getByRole("heading", { name: "Entropy", level: 1 }),
    ).toBeVisible();
    await expect(page.locator(".glossary")).toHaveCount(0);
  });

  test("the opened note is a history entry the way back undoes", async ({ page }) => {
    await page.goto("/?view=glossary");
    await page.getByRole("link", { name: "the prior" }).click();
    await expect(page.getByRole("heading", { name: "Prior" })).toBeVisible();

    await page.goBack();
    await expect(page.getByText(DEFINITION)).toBeVisible();
  });
});

test.describe("a glossary longer than one page", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, LONG);
  });

  test("browses to its last term past the cut it states", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByRole("option", { name: "Term 25" })).toBeVisible();

    await expect(
      page.getByText(`${PAGE} of ${LONG.notes.length} terms read.`),
    ).toBeVisible();
    await expect(page.getByRole("option", { name: "Term 26" })).toHaveCount(0);

    await page.getByRole("button", { name: "Read more terms" }).click();

    await expect(page.getByRole("option", { name: "Term 30" })).toBeVisible();
    await expect(page.getByRole("option")).toHaveCount(30);
    // The listing is whole, so it claims no remainder and offers no way onward.
    await expect(page.getByText("terms read.")).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Read more terms" }),
    ).toHaveCount(0);
  });

  test("continues the list from the end of the list's own scrollport", async ({
    page,
  }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByRole("option", { name: "Term 25" })).toBeVisible();

    // The rows overflow their box, which is the geometry this drives: the list
    // scrolls, the page does not, so the page's end would never come.
    const list = page.locator(".glossary-terms");
    expect(
      await list.evaluate((box) => box.scrollHeight - box.clientHeight),
    ).toBeGreaterThan(0);

    await list.evaluate((box) => box.scrollTo(0, box.scrollHeight));

    await expect(page.getByRole("option", { name: "Term 30" })).toBeVisible();
    await expect(page.getByRole("option")).toHaveCount(LONG.notes.length);
  });

  test("holds the peeked term across the page that follows it", async ({ page }) => {
    await page.goto("/?view=glossary");
    // A row in view: reaching one further down would scroll the list to its end
    // and continue it, which is the other test's subject. Named exactly, since a
    // role name matches on a substring and the page to come holds Term 30.
    const marked = page.getByRole("option", { name: "Term 3", exact: true });
    await marked.click();
    await expect(marked).toHaveAttribute("aria-selected", "true");

    await page.getByRole("button", { name: "Read more terms" }).click();
    await expect(page.getByRole("option", { name: "Term 30" })).toBeVisible();

    // Rows arrive after the ones held and the peek is keyed by term, so the
    // definition beside the list is still the one the reader was reading.
    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(
      page.getByRole("heading", { name: "Term 3", level: 2, exact: true }),
    ).toBeVisible();
  });
});

test.describe("the glossary peek under a hoverless pointer", () => {
  // Without a touchscreen the browser reports `hover: hover`, and the tap below
  // would be an ordinary click taking the committing path instead.
  test.use({ hasTouch: true });

  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("a tap on a link in a definition opens it, having no card to commit from", async ({
    page,
  }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    await page.getByRole("link", { name: "the prior" }).tap();

    // In the reading column a tap raises a card carrying `Open` and `Replace`.
    // This surface mounts no card, so a tap that only glanced would be a link a
    // finger could never follow.
    await expect(page.locator(".glance-card")).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "Prior" })).toBeVisible();
  });

  test("the pane and its term list end at the visible viewport", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    // A coarse pointer raises `--header-min-height` to what the bar then
    // measures, so this is the branch where the pane's `calc(viewport - token)`
    // has the taller header to subtract.
    const geometry = await page.evaluate(() => {
      const bottomOf = (selector: string): number =>
        (document.querySelector(selector) as HTMLElement).getBoundingClientRect()
          .bottom;
      return {
        visibleViewport: Math.round(window.visualViewport?.height ?? 0),
        paneBottom: Math.round(bottomOf(".glossary")),
        termsBottom: Math.round(bottomOf(".glossary-terms")),
        pageScrollHeight: document.documentElement.scrollHeight,
      };
    });

    expect(geometry.paneBottom).toBe(geometry.visibleViewport);
    expect(geometry.termsBottom).toBeLessThanOrEqual(geometry.visibleViewport);
    expect(geometry.pageScrollHeight).toBeLessThanOrEqual(geometry.visibleViewport);
  });

  test("the term's own control clears the touch-target floor", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    // The floor comes off `min-height`, which an inline box ignores, so this
    // control has to be laid out as one that does not.
    const box = await page
      .getByRole("link", { name: "Open in reader" })
      .boundingBox();
    expect(box!.height).toBeGreaterThanOrEqual(TOUCH_TARGET);
  });
});
