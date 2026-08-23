
import { expect, test } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

/**
 * A mutually linked pair. Each note's key and id differ in spelling, so a stack
 * holding `file:flows.org` and a link naming `id:flows-uuid` are one note under
 * two names.
 */
const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:flows.org",
      id: "flows-uuid",
      title: "Normalizing flows",
      body: "Built on [[id:change-uuid][change of variables]].",
      forwardLinks: [
        {
          key: "file:change.org",
          id: "change-uuid",
          title: "Change of variables",
          preview: "the density transform",
        },
      ],
    },
    {
      key: "file:change.org",
      id: "change-uuid",
      title: "Change of variables",
      body: "The foundation of [[id:flows-uuid][normalizing flows]].",
      backlinks: [
        {
          key: "file:flows.org",
          id: "flows-uuid",
          title: "Normalizing flows",
          preview: "built on change of variables",
        },
      ],
    },
  ],
};

/**
 * The in-body link in the frontmost column. Selected by class rather than by
 * accessible name: a note's body link and its relation row carry the same title,
 * and role-name matching is case-insensitive, so a name alone matches both.
 */
const bodyLink = (page: import("@playwright/test").Page) =>
  page.locator(".spine-column").last().locator("a.org-link");

test.describe("distinct columns", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("following a link back to the open root reveals it, not a copy", async ({
    page,
  }) => {
    await page.goto("/?note=file:flows.org");
    await expect(page.getByRole("heading", { name: "Normalizing flows" })).toBeVisible();
    await bodyLink(page).click();
    await expect(
      page.getByRole("heading", { name: "Change of variables" }),
    ).toBeVisible();
    expect(await page.locator(".spine-column").count()).toBe(2);

    // The neighbor's link targets `id:flows-uuid`, the note the root holds as
    // `file:flows.org`. Each column stays named by the reference that opened it.
    await bodyLink(page).click();
    expect(await page.locator(".spine-column").count()).toBe(2);
    await expect(page).toHaveURL(
      `/?note=${encodeURIComponent("file:flows.org")}&stacked=${encodeURIComponent("id:change-uuid")}`,
    );

    const root = page.getByRole("heading", { name: "Normalizing flows" });
    await expect(root).toBeVisible();
    await expect(root).toBeInViewport();
  });

  test("revealing an open note adds no history entry", async ({ page }) => {
    await page.goto("/?note=file:flows.org");
    await bodyLink(page).click();
    await expect(
      page.getByRole("heading", { name: "Change of variables" }),
    ).toBeVisible();

    await bodyLink(page).click();
    await page.goBack();

    await expect(page).toHaveURL("/?note=file:flows.org");
    await expect(page.getByRole("heading", { name: "Change of variables" })).toHaveCount(
      0,
    );
  });

  test("an address repeating a note opens one column, where it first stands", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(page.getByRole("combobox", { name: "Search the slipbox" })).toBeVisible();
    const collapsed = `/?note=${encodeURIComponent("file:flows.org")}&stacked=${encodeURIComponent("file:change.org")}`;
    await page.goto(`${collapsed}&stacked=${encodeURIComponent("file:flows.org")}`);

    await expect(page.locator(".reading-note__title")).toHaveText([
      "Normalizing flows",
      "Change of variables",
    ]);
    await expect(page).toHaveURL(collapsed);

    await page.goBack();
    await expect(page.getByRole("combobox", { name: "Search the slipbox" })).toBeVisible();
  });

  test("a relation row to an open note reveals it too", async ({ page }) => {
    await page.goto("/?note=file:flows.org&stacked=file:change.org");
    await expect(
      page.getByRole("heading", { name: "Change of variables" }),
    ).toBeVisible();

    // The mount's own reveal scroll is issued from an animation frame, so this
    // polls for the root to be partly covered rather than racing that frame.
    await expect(async () => {
      expect(await clearWidthOf(page, 0)).toBeLessThan(600);
    }).toPass();

    await page
      .locator(".spine-column")
      .last()
      .locator("a.relations__link")
      .first()
      .click();

    expect(await page.locator(".spine-column").count()).toBe(2);
    // 625 is the full column width: the root stands entirely clear. Polled,
    // since the reveal scroll runs from an animation frame.
    await expect(async () => {
      expect(await clearWidthOf(page, 0)).toBe(625);
    }).toPass();
  });
});

/**
 * How much of column `index` is visible: its width inside the viewport, less
 * whatever the next column overlaps. A spine holding two full-width columns in
 * a 1200px frame renders both headings whether or not either is scrolled to, so
 * this geometry is what a reveal actually moves.
 */
async function clearWidthOf(
  page: import("@playwright/test").Page,
  index: number,
): Promise<number> {
  return page.evaluate((which) => {
    const columns = [...document.querySelectorAll(".spine-column")];
    const rect = columns[which]!.getBoundingClientRect();
    const next = columns[which + 1]?.getBoundingClientRect();
    const right = Math.min(rect.right, window.innerWidth, next?.left ?? Infinity);
    return Math.round(Math.max(0, right - Math.max(rect.left, 0)));
  }, index);
}
