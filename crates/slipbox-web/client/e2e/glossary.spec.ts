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
    // Marked from the keyboard and continued by scrolling the box, so the pointer
    // never rests over the list: rows arriving move the ones under a resting
    // pointer, and hovering one is a selection like any other. Named exactly,
    // since a role name matches on a substring and the page to come holds Term 30.
    const marked = page.getByRole("option", { name: "Term 3", exact: true });
    // The first row is peeked on arrival, so two rows down is the third.
    const field = page.getByRole("combobox");
    await field.press("ArrowDown");
    await field.press("ArrowDown");
    await expect(marked).toHaveAttribute("aria-selected", "true");

    await page
      .locator(".glossary-terms")
      .evaluate((box) => box.scrollTo(0, box.scrollHeight));
    await expect(page.getByRole("option", { name: "Term 30" })).toBeVisible();

    // Rows arrive after the ones held and the peek is keyed by term, so the
    // definition beside the list is still the one the reader was reading.
    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(
      page.getByRole("heading", { name: "Term 3", level: 2, exact: true }),
    ).toBeVisible();
  });
});

test.describe("the glossary term in the address", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, LONG);
  });

  test("a copied address reopens the definition it names", async ({ page }) => {
    await page.goto("/?view=glossary&term=file%3Aterm-3.org");

    // Named exactly: a role name matches on a substring, and this list holds a
    // Term 30 the pattern would otherwise reach too.
    const marked = page.getByRole("option", { name: "Term 3", exact: true });
    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(
      page.getByRole("heading", { name: "Term 3", level: 2, exact: true }),
    ).toBeVisible();
    await expect(page.getByText("Definition 3.")).toBeVisible();
  });

  test("a reload holds the open term, which cost no history entry", async ({
    page,
  }) => {
    await page.goto("/?view=glossary");
    const marked = page.getByRole("option", { name: "Term 3", exact: true });
    const entries = await page.evaluate(() => history.length);

    await marked.click();
    await expect(page.getByText("Definition 3.")).toBeVisible();

    // The address carries the term beside the surface's own parameter rather
    // than in place of it, and replaces the entry rather than adding one: the
    // way back out of the glossary is the way in, not a walk back through every
    // term peeked along the way.
    expect(new URL(page.url()).search).toBe(
      "?view=glossary&term=file%3Aterm-3.org",
    );
    expect(await page.evaluate(() => history.length)).toBe(entries);

    await page.reload();

    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(page.getByText("Definition 3.")).toBeVisible();
  });

  test("leaves the term on the entry it came from when the surface changes", async ({
    page,
  }) => {
    await page.goto("/?view=glossary");
    const marked = page.getByRole("option", { name: "Term 3", exact: true });
    await marked.click();
    await expect(page.getByText("Definition 3.")).toBeVisible();

    await page.getByRole("button", { name: "Notes" }).click();

    // The term names a row of a term listing, so it goes off the address of a
    // surface that has none. Off the entry being pushed, not the one being left:
    // the way back is a glossary still showing the definition it was showing.
    await expect(page.getByRole("combobox", { name: "Search notes" })).toBeVisible();
    expect(new URL(page.url()).search).toBe("");

    await page.goBack();

    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(page.getByText("Definition 3.")).toBeVisible();
  });

  test("an address naming a term past the page read reads it anyway", async ({
    page,
  }) => {
    await page.goto("/?view=glossary&term=file%3Aterm-28.org");

    // Off the first page, so the list cannot mark it: the term is read on its
    // own and the list says why the row is nowhere to be found.
    await expect(page.getByText("Definition 28.")).toBeVisible();
    await expect(
      page.getByText("Term 28 is not among the terms read so far."),
    ).toBeVisible();

    // Scrolled rather than clicked, keeping the pointer off the list: the rows the
    // page brings move the ones a resting pointer is over, and hovering a row is
    // a selection, which would be this test marking a term of its own.
    await page
      .locator(".glossary-terms")
      .evaluate((box) => box.scrollTo(0, box.scrollHeight));

    const marked = page.getByRole("option", { name: "Term 28", exact: true });
    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(
      page.getByText("is not among the terms read so far."),
    ).toHaveCount(0);
  });

  test("an address naming no term says so and leaves the list usable", async ({
    page,
  }) => {
    await page.goto("/?view=glossary&term=file%3Amissing.org");

    await expect(
      page.getByText("No glossary term is named file:missing.org."),
    ).toBeVisible();
    await expect(page.getByText("Select a term to read its definition.")).toBeVisible();

    await page.getByRole("option", { name: "Term 1", exact: true }).click();

    await expect(page.getByText("Definition 1.")).toBeVisible();
    await expect(
      page.getByText("No glossary term is named file:missing.org."),
    ).toHaveCount(0);
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
