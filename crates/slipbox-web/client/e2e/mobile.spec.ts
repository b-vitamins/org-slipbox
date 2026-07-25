/*
 * The narrow layout below the 800px breakpoint, and the touch grammar.
 *
 * Only a real browser reports `hover: none` / `pointer: coarse` and synthesizes
 * a tap's mouse events, and only a real cascade gives a control the height the
 * touch-target assertions read, so neither surface is reachable from jsdom.
 */

import { expect, test, type Locator, type Page } from "@playwright/test";

import { mountApi, tallBody, type FixtureWorld } from "./fixtures.js";

/** The reading area's own top edge: where a revealed note has to begin. */
async function readerTop(page: Page): Promise<number> {
  const box = await page.locator(".spine").boundingBox();
  return box?.y ?? 0;
}

/** How far below the reading area's top edge a revealed title may sit. */
const REVEAL_SLACK = 8;

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:origin.org",
      title: "Origin Note",
      // Tall, so the pinned target lands well below the fold and must be
      // scrolled to rather than merely rendered.
      body: `${tallBody(40)}\n\nFollow [[id:pinned-uuid][the pinned note]] here.`,
      forwardLinks: [
        {
          key: "file:pinned.org",
          id: "pinned-uuid",
          title: "Pinned Note",
          preview: "the destination",
        },
        // Title and linking line are each wider than a 375px phone column, so
        // the footer row has to wrap.
        {
          key: "file:verbose.org",
          id: "verbose-uuid",
          title: "A deliberately long relation title, wider than a phone column",
          preview:
            "The linking line is long too, so title and preview cannot sit side by side.",
        },
      ],
    },
    {
      key: "file:pinned.org",
      id: "pinned-uuid",
      title: "Pinned Note",
      body: "The pinned target, which must render in full on mobile.",
    },
  ],
};

test.use({ viewport: { width: 375, height: 720 } });

test.describe("mobile layout", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("the narrow layout stacks columns vertically", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    // The breakpoint lives in the stylesheet, so the computed flex-direction is
    // the only place a script can read whether it engaged.
    const direction = await page.evaluate(
      () => getComputedStyle(document.querySelector(".spine") as HTMLElement).flexDirection,
    );
    expect(direction).toBe("column");
  });

  test("a note opened from its URL begins below the sticky header", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    const title = page.getByRole("heading", { name: "Origin Note" });
    await expect(title).toBeVisible();

    // The sticky header paints over the reading area, so a title scrolled up
    // beneath it still intersects the viewport: box positions are what separate
    // the two, not `toBeInViewport`.
    const header = await page.locator(".app-header").boundingBox();
    const heading = await title.boundingBox();
    expect(heading!.y).toBeGreaterThanOrEqual(header!.y + header!.height);
    expect(await page.locator(".spine").evaluate((node) => node.scrollTop)).toBe(0);
  });

  test("a pinned note renders in full and lands at the top of the reader", async ({
    page,
  }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    await page.getByRole("link", { name: "the pinned note" }).click();

    // The origin note is tall, so a heading that merely intersects the viewport
    // may be peeking in at the bottom: the column's top edge against the
    // reader's own is what pins the reveal.
    const pinned = page.getByRole("heading", { name: "Pinned Note" });
    await expect(pinned).toBeVisible();
    const column = await page.locator(".spine-column").nth(1).boundingBox();
    expect(column!.y).toBeGreaterThanOrEqual(await readerTop(page));
    expect(column!.y).toBeLessThanOrEqual((await readerTop(page)) + REVEAL_SLACK);
    await expect(
      page.getByText("The pinned target, which must render in full on mobile."),
    ).toBeVisible();
  });

  test("no obscured sliver survives the narrow layout", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    await page.getByRole("link", { name: "the pinned note" }).click();
    await expect(page.getByRole("heading", { name: "Pinned Note" })).toBeVisible();

    await expect(page.locator("button.reading-note--obscured")).toHaveCount(0);

    // The stylesheet hides the affordance independently of the geometry. The
    // layout renders no such element, so the rule is read off a probe carrying
    // the class.
    const hidden = await page.evaluate(() => {
      const probe = document.createElement("button");
      probe.className = "reading-note reading-note--obscured";
      document.querySelector(".spine-column")!.append(probe);
      const display = getComputedStyle(probe).display;
      probe.remove();
      return display;
    });
    expect(hidden).toBe("none");
  });

  test("a relation row stays inside the column it belongs to", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    // The narrow layout scrolls vertically only, so horizontal overflow is
    // unreachable text rather than something a reader can scroll to.
    const note = await page.locator(".spine-column").first().locator(".reading-note").boundingBox();
    const row = await page
      .locator(".relations__row")
      .filter({ hasText: "wider than a phone column" })
      .boundingBox();
    expect(row!.x).toBeGreaterThanOrEqual(note!.x);
    expect(row!.x + row!.width).toBeLessThanOrEqual(note!.x + note!.width);

    const preview = await page
      .locator(".relations__row")
      .filter({ hasText: "wider than a phone column" })
      .locator(".relations__preview")
      .boundingBox();
    expect(preview!.width).toBeGreaterThan(0);
  });

  test("an empty glossary explains itself inside the phone's width", async ({
    page,
  }) => {
    await page.goto("/");
    await page.getByRole("button", { name: "Glossary" }).click();

    // Nothing in WORLD is marked, so the glossary is in its empty state.
    await expect(
      page.getByRole("heading", { name: "How terms enter the glossary" }),
    ).toBeVisible();

    // These literals are wider than a 375px column, which scrolls vertically
    // only, so unwrapped they run off an edge no reader can reach.
    const pane = await page.locator(".glossary-empty").boundingBox();
    for (const literal of ["#+glossary: t", "slipbox glossary mark"]) {
      const box = await page.getByText(literal, { exact: true }).boundingBox();
      expect(box!.x).toBeGreaterThanOrEqual(pane!.x);
      expect(box!.x + box!.width).toBeLessThanOrEqual(pane!.x + pane!.width);
    }
    expect(pane!.x + pane!.width).toBeLessThanOrEqual(375);
  });
});
