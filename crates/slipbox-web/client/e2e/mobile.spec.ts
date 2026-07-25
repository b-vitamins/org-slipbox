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

async function heightOf(control: Locator): Promise<number> {
  const box = await control.boundingBox();
  return box?.height ?? 0;
}

/** The `min-height` the stylesheet gives a control under a coarse pointer. */
const TOUCH_TARGET = 44;

/**
 * A key WORLD does not model, so the read is a genuine 404. The stem is words
 * rather than a uuid, which is what makes a search out of it offered at all.
 */
const MISSING_NOTE = "file:renamed-note.org";

test.describe("the touch grammar", () => {
  // Without a touchscreen the browser reports `hover: hover` / `pointer: fine`
  // and this frame is only a narrow desktop, so every assertion below would
  // pass for the wrong reason.
  test.use({ hasTouch: true });

  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("a tap previews the target and the preview commits it", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    await page.getByRole("link", { name: "the pinned note" }).tap();
    const card = page.locator(".glance-card--committing");
    await expect(card).toBeVisible();
    await expect(card.getByText("Pinned Note")).toBeVisible();
    await expect(page.locator(".spine-column")).toHaveCount(1);

    // A finger has no second gesture and no Alt key, so the committing verbs
    // have to be on the card itself.
    await expect(card.getByRole("button", { name: "Open" })).toBeVisible();
    await expect(card.getByRole("button", { name: "Replace" })).toBeVisible();
    await expect(card.getByRole("button", { name: "Close" })).toBeVisible();

    await card.getByRole("button", { name: "Open" }).tap();
    await expect(page.locator(".spine-column")).toHaveCount(2);
    await expect(page.getByRole("heading", { name: "Pinned Note" })).toBeVisible();
    await expect(page.locator(".glance-card")).toHaveCount(0);
  });

  test("a peek can be closed without committing to anything", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    await page.getByRole("link", { name: "the pinned note" }).tap();

    // A cursor dismisses a glance by moving away; a finger has nowhere to move
    // to, so the card carries the way out.
    await page.locator(".glance-card").getByRole("button", { name: "Close" }).tap();
    await expect(page.locator(".glance-card")).toHaveCount(0);
    await expect(page.locator(".spine-column")).toHaveCount(1);
  });

  test("every control clears the touch-target floor", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Surprise me" })).toBeVisible();

    for (const control of [
      page.getByRole("button", { name: /^Color scheme:/ }),
      page.getByRole("button", { name: "Notes" }),
      page.getByRole("button", { name: "Glossary" }),
      page.getByRole("combobox", { name: "Search notes" }),
      page.getByRole("button", { name: "Surprise me" }),
    ]) {
      expect(await heightOf(control)).toBeGreaterThanOrEqual(TOUCH_TARGET);
    }

    await page.getByRole("button", { name: "Glossary" }).tap();
    for (const control of [
      page.getByRole("tab", { name: "All terms" }),
      page.getByRole("tab", { name: "Due for review" }),
      page.getByRole("combobox", { name: "Search the glossary" }),
    ]) {
      expect(await heightOf(control)).toBeGreaterThanOrEqual(TOUCH_TARGET);
    }

    // In the reading column a control is usually an anchor laid out as a box, so
    // those are sampled beside the buttons.
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();
    for (const control of [
      page.getByRole("link", { name: "Pinned Note", exact: true }),
      page.getByRole("link", { name: /^A deliberately long relation title/ }),
      page.locator(".neighborhood__toggle"),
    ]) {
      expect(await heightOf(control)).toBeGreaterThanOrEqual(TOUCH_TARGET);
    }

    // The fixture neighborhood walk answers with no nodes, so the opened
    // disclosure renders no member to measure: the height comes off a probe
    // carrying the classes the ring builds.
    await page.locator(".neighborhood__toggle").tap();
    const member = await page.evaluate(() => {
      const list = document.createElement("ul");
      list.className = "neighborhood__members";
      list.innerHTML =
        '<li class="neighborhood__member">' +
        '<a class="neighborhood__link" href="?note=file:pinned.org">Near Note</a>' +
        "</li>";
      document.querySelector(".neighborhood")!.append(list);
      const height = list.querySelector("a")!.getBoundingClientRect().height;
      list.remove();
      return height;
    });
    expect(member).toBeGreaterThanOrEqual(TOUCH_TARGET);

    await page.goto(`/?note=${MISSING_NOTE}`);
    const search = page.getByRole("link", { name: /^Search the slipbox for/ });
    await expect(search).toBeVisible();
    expect(await heightOf(search)).toBeGreaterThanOrEqual(TOUCH_TARGET);
  });

  test("a link in body prose is left the width of its own words", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    // `min-height` does nothing to an inline box, which is what keeps the floor
    // off a link a sentence runs through. The `display` read is what pins that:
    // a link laid out any other way would take the 44px and open a gap through
    // the middle of the line.
    const link = page.getByRole("link", { name: "the pinned note" });
    expect(await link.evaluate((node) => getComputedStyle(node).display)).toBe(
      "inline",
    );
    expect(await heightOf(link)).toBeLessThan(TOUCH_TARGET);

    const linking = page.locator(".org-paragraph").filter({ hasText: "Follow" });
    expect(await heightOf(linking)).toBeLessThan(TOUCH_TARGET);
  });
});
