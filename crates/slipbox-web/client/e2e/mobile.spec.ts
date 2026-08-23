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

/** The opening words of the body's first paragraph, which the measure is read on. */
const FIRST_PARAGRAPH = "Paragraph 1.";

/**
 * How many characters of a paragraph's first rendered line fit before it turns.
 * Where the text breaks is the browser's decision and nothing in the DOM records
 * it, so the break is found by walking a range to the character whose box drops
 * to the next line.
 */
function charactersPerLine(page: Page, opening: string): Promise<number> {
  return page.evaluate((prefix) => {
    const paragraph = Array.from(document.querySelectorAll(".org-paragraph")).find(
      (node) => (node.textContent ?? "").startsWith(prefix),
    );
    const text = paragraph?.firstChild;
    if (!(text instanceof Text)) {
      throw new Error(`no paragraph opening ${prefix}`);
    }
    const range = document.createRange();
    range.setStart(text, 0);
    range.setEnd(text, 1);
    const firstLine = range.getBoundingClientRect().top;
    for (let at = 2; at <= text.length; at += 1) {
      range.setEnd(text, at);
      const rects = Array.from(range.getClientRects());
      const last = rects[rects.length - 1];
      if (last && last.top > firstLine + 1) {
        return at - 1;
      }
    }
    return text.length;
  }, opening);
}

/** The note box, the column holding it, and the width the column has to fill. */
function noteGeometry(page: Page) {
  return page.evaluate(() => {
    const column = document.querySelector(".spine-column") as HTMLElement;
    const note = column.querySelector(".reading-note") as HTMLElement;
    const paragraph = column.querySelector(".org-paragraph") as HTMLElement;
    const style = getComputedStyle(note);
    return {
      run: Math.round((document.querySelector(".spine") as HTMLElement).clientWidth),
      column: Math.round(column.getBoundingClientRect().width),
      columnLeft: Math.round(column.getBoundingClientRect().left),
      note: Math.round(note.getBoundingClientRect().width),
      noteLeft: Math.round(note.getBoundingClientRect().left),
      paragraph: Math.round(paragraph.getBoundingClientRect().width),
      overflow: Math.round(
        (document.querySelector(".spine") as HTMLElement).scrollWidth -
          (document.querySelector(".spine") as HTMLElement).clientWidth,
      ),
      padding:
        Math.round(Number.parseFloat(style.paddingLeft)) +
        Math.round(Number.parseFloat(style.paddingRight)),
    };
  });
}

/**
 * The measure a relation preview keeps, in `ch`, which the stylesheet states as
 * its floor: about 45 characters of prose, and the point below which a clipped
 * line stops distinguishing the notes it belongs to.
 */
const PREVIEW_MEASURE = 30;

/** Rows a deferred group is given: more than it shows, so it holds some back. */
const GROUP_ROWS = 12;

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
      ],
      // A row's preview quotes the other note, so a row that has to wrap around
      // one is an inbound row.
      backlinks: [
        // Title and linking line are each wider than a 375px column, so the
        // footer row has to wrap.
        {
          key: "file:verbose.org",
          id: "verbose-uuid",
          title: "A deliberately long relation title, wider than a phone column",
          preview:
            "The linking line is long too, so title and preview cannot sit side by side.",
        },
        // A middling title, which leaves the preview room for a fragment but not
        // for a measure: the case the preview's own floor answers.
        {
          key: "file:middling.org",
          id: "middling-uuid",
          title: "Middling Note",
          preview: "The linking line is long enough to be clipped at any width.",
        },
      ],
      // Well over either deferred group's head, so an opened group carries both
      // kinds of control a closed one keeps out of reach: its rows, and the offer
      // of the rest. Nothing here reads the heads themselves, since a group that
      // stopped holding anything back would fail on the offer being gone.
      bridges: Array.from({ length: GROUP_ROWS }, (_, at) => ({
        key: `file:bridged-${at}.org`,
        title: `Bridged Note ${at + 1}`,
        via: [{ key: "file:middling.org", title: "Middling Note" }],
      })),
      mentions: Array.from({ length: GROUP_ROWS }, (_, at) => ({
        source: { key: `file:naming-${at}.org`, title: `Naming Note ${at + 1}` },
        line: "Origin Note is named here, and not linked to.",
        matched: "Origin Note",
      })),
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

  test("the narrow trail names the position and moves between notes", async ({
    page,
  }) => {
    await page.goto("/?note=file:origin.org&stacked=file:pinned.org");

    const secondTrail = page.locator(".spine-position").nth(1);
    await expect(secondTrail.getByText("Note 2 of 2")).toHaveAttribute(
      "aria-current",
      "step",
    );
    await secondTrail.getByRole("button", { name: "Previous" }).click();

    const firstTrail = page.locator(".spine-position").first();
    await expect(firstTrail.getByText("Note 1 of 2")).toHaveAttribute(
      "aria-current",
      "step",
    );
    const column = await page.locator(".spine-column").first().boundingBox();
    expect(column!.y).toBeGreaterThanOrEqual(await readerTop(page));
    expect(column!.y).toBeLessThanOrEqual((await readerTop(page)) + REVEAL_SLACK);
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

  test("at 375px the note still fills the width, less its padding", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    // 375px is narrower than the measure, so the cap that holds the line length
    // at a wide narrow viewport has to be inert here: every pixel the frame has
    // is one the note needs.
    const geometry = await noteGeometry(page);
    expect(geometry.column).toBe(geometry.run);
    expect(geometry.note).toBe(geometry.run);
    expect(geometry.paragraph).toBe(geometry.note - geometry.padding);
  });

  test("a preview keeps a legible measure or takes a line of its own", async ({
    page,
  }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    const rows = await page.evaluate(() => {
      const list = document.querySelector(".relations__list") as HTMLElement;
      // `ch` is the preview's own unit, so it is read off the preview's font
      // rather than assumed from a size. The ruler carries the class for that
      // font and drops the floor stated in it, which is what it is measuring.
      const ruler = document.createElement("span");
      ruler.className = "relations__preview";
      ruler.style.cssText =
        "position:absolute;visibility:hidden;width:1ch;min-width:0";
      list.append(ruler);
      const ch = ruler.getBoundingClientRect().width;
      ruler.remove();

      const listed = list.getBoundingClientRect();
      return [...document.querySelectorAll(".relations__row")]
        .map((row) => ({
          link: row.querySelector(".relations__link") as HTMLElement,
          preview: row.querySelector(".relations__preview") as HTMLElement | null,
        }))
        .filter((row) => row.preview !== null)
        .map((row) => {
          const preview = row.preview as HTMLElement;
          const line = parseFloat(getComputedStyle(preview).lineHeight);
          const title = row.link.getBoundingClientRect();
          const box = preview.getBoundingClientRect();
          return {
            measure: box.width / ch,
            lines: box.height / line,
            shares: Math.abs(box.y - title.y) < line,
            width: box.width,
            // What the row's own line leaves the preview from where it starts.
            room: listed.x + listed.width - box.x,
          };
        });
    });

    expect(rows.length).toBeGreaterThan(1);
    for (const row of rows) {
      // A fragment shorter than this cannot tell one linking line from another,
      // so the preview holds the measure whichever line it ends up on.
      expect(row.measure).toBeGreaterThanOrEqual(PREVIEW_MEASURE);
      // Still one clipped line, never a wrapped paragraph.
      expect(row.lines).toBeLessThanOrEqual(1);
      // Still inside a column that scrolls vertically only.
      expect(row.width).toBeLessThanOrEqual(row.room);
      // A preview the title left no measure for takes the whole line instead.
      if (!row.shares) {
        expect(row.width).toBeGreaterThanOrEqual(row.room - 1);
      }
    }
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

/*
 * At 720px the run is stacked but the frame is still wide, so a full-width
 * column offers a line far more room than prose can use. Its own viewport, since
 * the block above pins the whole file to 375px.
 */
test.describe("the reading measure in a stacked column", () => {
  test.use({ viewport: { width: 720, height: 900 } });

  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("a stacked note reads at the measure a column gives it", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    const stacked = await charactersPerLine(page, FIRST_PARAGRAPH);
    const geometry = await noteGeometry(page);

    // The note is inset while the column still spans the run, so the border along
    // a column's top edge reaches both edges of the frame.
    expect(geometry.column).toBe(geometry.run);
    expect(geometry.note).toBeLessThan(geometry.run);
    expect(geometry.overflow).toBe(0);
    expect(geometry.noteLeft).toBeGreaterThan(geometry.columnLeft);
    const trailing =
      geometry.columnLeft + geometry.column - (geometry.noteLeft + geometry.note);
    expect(
      Math.abs(geometry.noteLeft - geometry.columnLeft - trailing),
    ).toBeLessThanOrEqual(1);

    // The same paragraph in the layout the measure is taken from, measured rather
    // than assumed, so both readings are one text in one font.
    await page.setViewportSize({ width: 1200, height: 900 });
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            getComputedStyle(document.querySelector(".spine") as HTMLElement)
              .flexDirection,
        ),
      )
      .toBe("row");
    const inColumn = await charactersPerLine(page, FIRST_PARAGRAPH);

    expect(stacked).toBeLessThanOrEqual(inColumn);
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
    ]) {
      expect(await heightOf(control)).toBeGreaterThanOrEqual(TOUCH_TARGET);
    }

    await page.goto(`/?note=${MISSING_NOTE}`);
    const search = page.getByRole("link", { name: /^Search the slipbox for/ });
    await expect(search).toBeVisible();
    expect(await heightOf(search)).toBeGreaterThanOrEqual(TOUCH_TARGET);
  });

  // A deferred group keeps its rows and the offer of the rest behind one control,
  // so a sweep that reads the footer as it stands measures the group's label and
  // nothing else it holds.
  test("a deferred group's controls clear the floor, open as well as shut", async ({
    page,
  }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    for (const label of ["Related notes", "Unlinked mentions"]) {
      const toggle = page.getByRole("button", { name: label });
      expect(await heightOf(toggle), `the ${label} label`).toBeGreaterThanOrEqual(
        TOUCH_TARGET,
      );

      await toggle.tap();
      await expect(toggle).toHaveAttribute("aria-expanded", "true");
      const group = page
        .locator(".relations__group")
        .filter({ has: page.getByRole("button", { name: label }) });

      const row = group.locator(".relations__link").first();
      await expect(row).toBeVisible();
      expect(await heightOf(row), `a row in ${label}`).toBeGreaterThanOrEqual(
        TOUCH_TARGET,
      );

      // The fixtures stand one row over the head, so the rest is offered rather
      // than shown, and the offer is a control of its own.
      const more = group.getByRole("button", { name: /^Show \d+ more$/ });
      await expect(more).toBeVisible();
      expect(await heightOf(more), `the offer in ${label}`).toBeGreaterThanOrEqual(
        TOUCH_TARGET,
      );
    }
  });

  test("a relation row takes its floor from the touch-target token", async ({ page }) => {
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin Note" })).toBeVisible();

    const link = page.getByRole("link", { name: /^A deliberately long relation title/ });
    expect(await heightOf(link)).toBeGreaterThanOrEqual(TOUCH_TARGET);

    // A row set at a height of its own would clear the constant above and then
    // stop clearing it here. Raising the token is what says the floor is still
    // the token after the row's own padding was cut to an index's.
    const RAISED = TOUCH_TARGET + 16;
    await page.evaluate((raised) => {
      document.documentElement.style.setProperty("--touch-target", `${raised}px`);
    }, RAISED);
    expect(await heightOf(link)).toBeGreaterThanOrEqual(RAISED);
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
