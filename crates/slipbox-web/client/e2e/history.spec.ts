/*
 * jsdom drives the stack reducer with an in-memory history, so the real
 * `pushState`/`popstate` wiring, history-entry state, restored focus, and
 * restored scroll are only reachable from a browser.
 */

import { expect, test } from "@playwright/test";

import { mountApi, tallBody, type FixtureWorld } from "./fixtures.js";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:root.org",
      title: "Root Note",
      body: "The root note links to [[id:child-uuid][the child]].",
      forwardLinks: [
        {
          key: "heading:child.org::1",
          id: "child-uuid",
          title: "Child Note",
          preview: "linked from the root",
        },
      ],
    },
    {
      key: "heading:child.org::1",
      id: "child-uuid",
      title: "Child Note",
      body: "The child, opened by pinning a link.",
    },
  ],
};

/** More rows than one screenful, so the last are reachable only by arrowing. */
const RESULT_COUNT = 18;

/** Every note matches "gradient", so a search on it lists all RESULT_COUNT. */
const LIST_WORLD: FixtureWorld = {
  notes: Array.from({ length: RESULT_COUNT }, (_, i) => ({
    key: `file:gradient-${i + 1}.org`,
    title: `Gradient note ${i + 1}`,
    body: tallBody(2),
  })),
};

test.describe("browser history", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("pinning a link pushes history that Back and Forward walk", async ({ page }) => {
    await page.goto("/?note=file:root.org");
    await expect(page.getByRole("heading", { name: "Root Note" })).toBeVisible();

    await page.getByRole("link", { name: "the child" }).click();
    await expect(page.getByRole("heading", { name: "Child Note" })).toBeVisible();
    await expect(page).toHaveURL(/stacked=/);

    await page.goBack();
    await expect(page).not.toHaveURL(/stacked=/);
    await expect(page.getByRole("heading", { name: "Child Note" })).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "Root Note" })).toBeVisible();

    await page.goForward();
    await expect(page).toHaveURL(/stacked=/);
    await expect(page.getByRole("heading", { name: "Child Note" })).toBeVisible();
  });

  test("the wordmark Home exit leaves the spine and Back returns to it", async ({
    page,
  }) => {
    await page.goto("/?note=file:root.org");
    await expect(page.getByRole("heading", { name: "Root Note" })).toBeVisible();

    const home = page.getByRole("button", { name: "Back to entry" });
    await expect(home).toBeVisible();
    await home.click();

    await expect(page.getByRole("combobox", { name: "Search the slipbox" })).toBeVisible();
    await page.goBack();
    await expect(page.getByRole("heading", { name: "Root Note" })).toBeVisible();
  });
});

test.describe("returning to a search", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, LIST_WORLD);
  });

  test("Back restores the arrowed-to result, in view and arrowable on", async ({
    page,
  }) => {
    // Short enough that the far rows sit off screen, so the restored highlight
    // has somewhere to be scrolled back from.
    await page.setViewportSize({ width: 375, height: 600 });
    await page.goto("/");

    const field = page.getByRole("combobox", { name: "Search the slipbox" });
    await field.fill("gradient");
    await expect(page.getByRole("option")).toHaveCount(RESULT_COUNT);

    for (let i = 0; i < RESULT_COUNT; i += 1) {
      await field.press("ArrowDown");
    }
    const last = page.getByRole("option", { name: /Gradient note 18/ });
    await expect(last).toHaveAttribute("aria-selected", "true");
    const scrolled = await page.evaluate(() => window.scrollY);
    expect(scrolled).toBeGreaterThan(0);

    await field.press("Enter");
    await expect(
      page.getByRole("heading", { name: "Gradient note 18" }),
    ).toBeVisible();

    await page.goBack();
    const restoredField = page.getByRole("combobox", { name: "Search the slipbox" });
    await expect(restoredField).toHaveValue("gradient");
    const restored = page.getByRole("option", { name: /Gradient note 18/ });
    await expect(restored).toHaveAttribute("aria-selected", "true");
    await expect(restored).toBeInViewport();

    // Focus must be on the field, not the body a browser falls back to, or
    // ArrowUp scrolls the page instead of walking the list.
    await expect(restoredField).toBeFocused();
    await restoredField.press("ArrowUp");
    await expect(
      page.getByRole("option", { name: /Gradient note 17/ }),
    ).toHaveAttribute("aria-selected", "true");
  });

  test("the restored place is the reader's own, not part of the address", async ({
    page,
    context,
  }) => {
    await page.goto("/");

    const field = page.getByRole("combobox", { name: "Search the slipbox" });
    await field.fill("gradient");
    await expect(page.getByRole("option")).toHaveCount(RESULT_COUNT);
    await field.press("ArrowDown");
    await field.press("Enter");
    await expect(page.getByRole("heading", { name: "Gradient note 1" })).toBeVisible();

    // The cursor rides on the history entry rather than the address.
    await page.goBack();
    await expect(page).toHaveURL(/[?&]q=gradient/);
    await expect(page).not.toHaveURL(/gradient-1\.org/);
    await expect(page.getByRole("option").first()).toHaveAttribute(
      "aria-selected",
      "true",
    );

    // A second page is a fresh history, so the same URL carries no cursor there.
    const shared = await context.newPage();
    await mountApi(shared, LIST_WORLD);
    await shared.goto("/?q=gradient");
    await expect(shared.getByRole("option")).toHaveCount(RESULT_COUNT);
    await expect(
      shared.getByRole("combobox", { name: "Search the slipbox" }),
    ).not.toHaveAttribute("aria-activedescendant");
    await shared.close();
  });
});
