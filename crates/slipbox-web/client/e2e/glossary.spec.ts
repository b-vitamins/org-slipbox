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

  test("the opened note is a history entry the way back undoes", async ({ page }) => {
    await page.goto("/?view=glossary");
    await page.getByRole("link", { name: "the prior" }).click();
    await expect(page.getByRole("heading", { name: "Prior" })).toBeVisible();

    await page.goBack();
    await expect(page.getByText(DEFINITION)).toBeVisible();
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
});
