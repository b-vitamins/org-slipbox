/*
 * Every spine behavior here is real-layout (flex sizing, box geometry, scroll
 * offsets), which jsdom does not compute.
 */

import { expect, test, type Locator } from "@playwright/test";

import { mountApi, tallBody, type FixtureWorld } from "./fixtures.js";

/** A four-note trail; the first note is tall enough to force internal scroll. */
const WORLD: FixtureWorld = {
  notes: [
    { key: "file:one.org", title: "Note One", body: tallBody() },
    { key: "file:two.org", title: "Note Two", body: "The second note." },
    { key: "file:three.org", title: "Note Three", body: "The third note." },
    {
      key: "file:four.org",
      title: "The Frontmost Note",
      body: "The fourth and frontmost note of the trail.",
    },
  ],
};

/**
 * The same trail, each note linking forward to the next, so a spec can walk it
 * by clicking. Only an `id:` target carries the navigation grammar (a `file:`
 * link is a plain browser anchor, see RenderInline), so the notes are
 * id-addressable and link by id.
 */
const LINKED_WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:one.org",
      id: "id-one",
      title: "Note One",
      body: `${tallBody()}\n\nContinue to [[id:id-two][Note Two]].`,
    },
    {
      key: "file:two.org",
      id: "id-two",
      title: "Note Two",
      body: "The second note. Continue to [[id:id-three][Note Three]].",
    },
    {
      key: "file:three.org",
      id: "id-three",
      title: "Note Three",
      body: "The third note. Continue to [[id:id-four][The Frontmost Note]].",
    },
    {
      key: "file:four.org",
      id: "id-four",
      title: "The Frontmost Note",
      body: "The fourth and frontmost note of the trail.",
    },
  ],
};

const GLANCE_WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:one.org",
      id: "id-one",
      title: "Note One",
      body: "Continue to [[id:id-two][Note Two]].",
    },
    {
      key: "file:two.org",
      id: "id-two",
      title: "Note Two",
      body: "The second note.",
    },
  ],
};

/**
 * 32 columns is deeper than the pinned ladder can hold: at the config's 1200px
 * an unbounded ladder leaves every column past roughly the twenty-ninth a
 * sliver at any scroll offset.
 */
const DEEP_WORLD: FixtureWorld = {
  notes: Array.from({ length: 32 }, (_, index) => ({
    key: `file:deep-${index}.org`,
    title: `Deep Note ${index}`,
    body: `The body of deep note ${index}.`,
  })),
};

const appGeometry = () => {
  const box = (selector: string): DOMRect =>
    (document.querySelector(selector) as HTMLElement).getBoundingClientRect();
  return {
    visibleViewport: Math.round(window.visualViewport?.height ?? 0),
    appHeight: Math.round(box(".app").height),
    headerHeight: Math.round(box(".app-header").height),
    spineHeight: Math.round(box(".spine").height),
    spineBottom: Math.round(box(".spine").bottom),
    pageScrollHeight: document.documentElement.scrollHeight,
  };
};

/** The URL restoring the whole deep trail, frontmost last. */
const DEEP_TRAIL = `/?note=${DEEP_WORLD.notes[0]!.key}${DEEP_WORLD.notes
  .slice(1)
  .map((note) => `&stacked=${note.key}`)
  .join("")}`;

const restingOffsets = (page: import("@playwright/test").Page) =>
  page.evaluate(() => {
    const spine = document.querySelector(".spine") as HTMLElement;
    const reach = spine.scrollWidth - spine.clientWidth;
    const columns = Array.from(
      document.querySelectorAll(".spine-column"),
    ) as HTMLElement[];
    const width = columns[0]!.getBoundingClientRect().width;
    return columns.map((column, index) =>
      Math.round(
        Math.min(
          Math.max(index * width - Number.parseFloat(column.style.left || "0"), 0),
          reach,
        ),
      ),
    );
  });

const spineScrollLeft = (page: import("@playwright/test").Page) =>
  page.evaluate(() =>
    Math.round((document.querySelector(".spine") as HTMLElement).scrollLeft),
  );

test.describe("reading spine", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("a reading path narrower than the screen is centered in it", async ({
    page,
  }) => {
    await page.goto("/?note=file:one.org");
    await expect(page.getByRole("heading", { name: "Note One" })).toBeVisible();

    // One 625px column in a 1200px frame. Asserted as symmetry rather than an
    // absolute offset, so it holds at any viewport the column fits inside.
    const gaps = await page.evaluate(() => {
      const spine = (document.querySelector(".spine") as HTMLElement).getBoundingClientRect();
      const column = (
        document.querySelector(".spine-column") as HTMLElement
      ).getBoundingClientRect();
      return { left: column.left - spine.left, right: spine.right - column.right };
    });
    expect(gaps.left).toBeGreaterThan(100);
    expect(Math.abs(gaps.left - gaps.right)).toBeLessThanOrEqual(1);
  });

  test("keeps the revealed note when the layout crosses its breakpoint", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 801, height: 900 });
    await page.goto(
      "/?note=file:one.org&stacked=file:two.org&stacked=file:three.org",
    );
    await expect(page).toHaveTitle("Note Three — slipbox");

    const offsets = await restingOffsets(page);
    await page.locator(".spine").evaluate((element, left) => {
      element.scrollLeft = left;
    }, offsets[1]!);
    await expect(page).toHaveTitle("Note Two — slipbox");

    await page.setViewportSize({ width: 800, height: 900 });
    await expect
      .poll(() =>
        page.locator(".spine").evaluate((element) => getComputedStyle(element).flexDirection),
      )
      .toBe("column");
    await expect(
      page.locator('.spine-position__count[aria-current="step"]'),
    ).toHaveText("Note 2 of 3");
    await expect(page).toHaveTitle("Note Two — slipbox");

    await page.setViewportSize({ width: 801, height: 900 });
    await expect
      .poll(() =>
        page.locator(".spine").evaluate((element) => getComputedStyle(element).flexDirection),
      )
      .toBe("row");
    await expect(page).toHaveTitle("Note Two — slipbox");
    await expect(page.getByRole("heading", { name: "Note Two" })).toBeInViewport();
  });

  test("shrinks a glance into the gutter at 1280px", async ({ page }) => {
    await mountApi(page, GLANCE_WORLD);
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto("/?note=file:one.org");

    await page.getByRole("link", { name: "Note Two", exact: true }).hover();
    await expect(page.locator(".glance-card--placed")).toBeVisible();
    const geometry = await page.evaluate(() => {
      const column = document
        .querySelector(".spine-column")!
        .getBoundingClientRect();
      const card = document.querySelector(".glance-card")!.getBoundingClientRect();
      return {
        columnRight: column.right,
        cardLeft: card.left,
        cardRight: card.right,
        cardWidth: card.width,
        viewport: window.innerWidth,
      };
    });

    expect(geometry.cardLeft).toBeGreaterThanOrEqual(geometry.columnRight);
    expect(geometry.cardRight).toBeLessThanOrEqual(geometry.viewport - 12);
    expect(geometry.cardWidth).toBeGreaterThanOrEqual(280);
    expect(geometry.cardWidth).toBeLessThan(340);
  });

  test("a path wider than the screen starts at its first column", async ({ page }) => {
    await page.goto(
      "/?note=file:one.org&stacked=file:two.org&stacked=file:three.org&stacked=file:four.org",
    );
    await expect(page.getByRole("heading", { name: "The Frontmost Note" })).toBeVisible();

    const rootLeft = await page.evaluate(() => {
      const spine = document.querySelector(".spine") as HTMLElement;
      spine.scrollLeft = 0;
      void spine.scrollLeft;
      const column = document.querySelector(".spine-column") as HTMLElement;
      return Math.round(
        column.getBoundingClientRect().left - spine.getBoundingClientRect().left,
      );
    });
    expect(rootLeft).toBe(0);
  });

  test("a single tall note scrolls its own body, not the page", async ({ page }) => {
    await page.goto("/?note=file:one.org");
    await expect(page.getByRole("heading", { name: "Note One" })).toBeVisible();

    const layout = await page.evaluate(() => {
      const spine = document.querySelector(".spine") as HTMLElement;
      const column = document.querySelector(".spine-column") as HTMLElement;
      column.scrollTop = 300;
      return {
        viewport: window.innerHeight,
        spineHeight: Math.round(spine.getBoundingClientRect().height),
        pageScrollHeight: document.documentElement.scrollHeight,
        columnScrolls: column.scrollHeight > column.clientHeight,
        columnScrollTop: Math.round(column.scrollTop),
        windowScrollY: Math.round(window.scrollY),
      };
    });

    // The 100px slack covers the 44px header the spine sits under. Without the
    // flex `min-height: 0` the spine would be about 2200px here.
    expect(layout.spineHeight).toBeLessThanOrEqual(layout.viewport);
    expect(layout.spineHeight).toBeGreaterThan(layout.viewport - 100);
    expect(layout.pageScrollHeight).toBeLessThanOrEqual(layout.viewport + 1);
    expect(layout.columnScrolls).toBe(true);
    expect(layout.columnScrollTop).toBeGreaterThan(0);
    expect(layout.windowScrollY).toBe(0);
  });

  test("the app fits the visible viewport, header overshoot and all", async ({
    page,
  }) => {
    await page.goto("/?note=file:one.org");
    await expect(page.getByRole("heading", { name: "Note One" })).toBeVisible();

    const authored = await page.evaluate(appGeometry);
    expect(authored.appHeight).toBe(authored.visibleViewport);
    expect(authored.headerHeight + authored.spineHeight).toBe(authored.appHeight);
    expect(authored.spineBottom).toBe(authored.visibleViewport);
    expect(authored.pageScrollHeight).toBeLessThanOrEqual(authored.visibleViewport);

    await page.evaluate(() => {
      const header = document.querySelector(".app-header") as HTMLElement;
      header.style.minHeight = "120px";
    });
    const grown = await page.evaluate(appGeometry);
    expect(grown.headerHeight).toBe(120);
    expect(grown.appHeight).toBe(grown.visibleViewport);
    expect(grown.spineBottom).toBe(grown.visibleViewport);
    expect(grown.pageScrollHeight).toBeLessThanOrEqual(grown.visibleViewport);
  });

  test("a restored four-note trail reveals its frontmost note", async ({ page }) => {
    await page.goto(
      "/?note=file:one.org&stacked=file:two.org&stacked=file:three.org&stacked=file:four.org",
    );

    // An obscured column renders a sliver button in place of its title, so a
    // visible title is itself proof the reveal scroll settled on this column.
    const frontmost = page.getByRole("heading", { name: "The Frontmost Note" });
    await expect(frontmost).toBeVisible();
    await expect(frontmost).toBeInViewport();
  });

  test("collapsed columns are focusable sliver buttons that reveal on click", async ({
    page,
  }) => {
    await page.goto(
      "/?note=file:one.org&stacked=file:two.org&stacked=file:three.org&stacked=file:four.org",
    );
    await expect(page.getByRole("heading", { name: "The Frontmost Note" })).toBeVisible();

    const slivers = page.locator("button.reading-note--obscured");
    await expect(slivers.first()).toBeVisible();
    const sliver = slivers.first();
    await sliver.focus();
    await expect(sliver).toBeFocused();

    // The first sliver holds the trail's root, so the expected label is spelled
    // out here rather than read back off the element under test.
    await expect(sliver).toHaveAttribute("aria-label", "Reveal Note One");

    // Measured against the column's own box rather than `toBeInViewport`, whose
    // default zero ratio a neighbouring column's heading also satisfies.
    await sliver.click();
    const revealed = page.getByRole("heading", { name: "Note One" });
    await expect(revealed).toBeVisible();
    const shown = await revealed.evaluate((heading) => {
      const column = heading.closest(".spine-column");
      const rect = (column as HTMLElement).getBoundingClientRect();
      return Math.round(
        Math.max(0, Math.min(rect.right, window.innerWidth) - Math.max(rect.left, 0)),
      );
    });
    // A sliver is about 40px wide, so 400 separates an open column from one
    // still held at its sliver.
    expect(shown).toBeGreaterThan(400);
  });

  test("hands focus to a revealed column, unpainted, sliver ring intact", async ({
    page,
  }) => {
    await page.goto(
      "/?note=file:one.org&stacked=file:two.org&stacked=file:three.org&stacked=file:four.org",
    );
    await expect(page.getByRole("heading", { name: "The Frontmost Note" })).toBeVisible();

    const frontmost = page.getByRole("article", { name: "The Frontmost Note" });
    await expect(frontmost).toBeFocused();
    const outlineOf = (target: Locator): Promise<string> =>
      target.evaluate((node) => getComputedStyle(node).outlineStyle);
    expect(await outlineOf(frontmost)).toBe("none");

    const sliver = page.locator("button.reading-note--obscured").first();
    await sliver.focus();
    expect(await outlineOf(sliver)).not.toBe("none");

    await sliver.click();
    const revealed = page.getByRole("article", { name: "Note One" });
    await expect(revealed).toBeFocused();
    expect(await outlineOf(revealed)).toBe("none");
  });

  test("a trail deeper than the pinned ladder still has a readable column", async ({
    page,
  }) => {
    await mountApi(page, DEEP_WORLD);
    await page.goto(DEEP_TRAIL);

    const columns = page.locator(".spine-column");
    await expect(columns).toHaveCount(DEEP_WORLD.notes.length);
    await expect(page.getByRole("heading", { name: "Deep Note 31" })).toBeVisible();

    const resting = await restingOffsets(page);
    const samples = await page.evaluate(async () => {
      const spine = document.querySelector(".spine") as HTMLElement;
      const settle = (): Promise<void> =>
        new Promise((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        );
      const reach = spine.scrollWidth - spine.clientWidth;
      const offsets: number[] = [];
      const counts: number[] = [];
      for (let step = 0; step <= 20; step += 1) {
        spine.scrollLeft = (reach * step) / 20;
        await settle();
        offsets.push(Math.round(spine.scrollLeft));
        counts.push(
          document.querySelectorAll(".spine-column:not(.spine-column--obscured)")
            .length,
        );
      }
      return { offsets, counts };
    });
    expect(Math.min(...samples.counts)).toBeGreaterThan(0);
    expect(samples.offsets.filter((offset) => !resting.includes(offset))).toEqual([]);
  });
});

/*
 * A wide frame with real motion, which the default config's 1200px
 * reduced-motion frame never enters.
 */
test.describe("reading spine (wide viewport, full motion)", () => {
  // `contextOptions` deep-merges over the config, so setting `reducedMotion`
  // here overrides the reduced default (an empty object would leave it reduced).
  test.use({
    viewport: { width: 1920, height: 1080 },
    contextOptions: { reducedMotion: "no-preference" },
  });

  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  const TRAIL =
    "/?note=file:one.org&stacked=file:two.org&stacked=file:three.org&stacked=file:four.org";

  // The visible width of the frontmost (last) column, clipped to the window. A
  // revealed column shows in full, one held at its sliver only about 40px.
  // Measured directly rather than inferred from `scrollWidth`, which reads full
  // at rest either way.
  const frontmostVisibleWidth = (page: import("@playwright/test").Page) =>
    page.evaluate(() => {
      const columns = document.querySelectorAll(".spine-column");
      const front = columns[columns.length - 1];
      if (!front) return 0;
      const rect = front.getBoundingClientRect();
      return Math.round(
        Math.max(0, Math.min(rect.right, window.innerWidth) - Math.max(rect.left, 0)),
      );
    });

  // Resolve once the spine's horizontal scroll has come to rest, so the
  // frontmost is measured where it settles rather than mid-animation. The 700ms
  // floor keeps the pre-animation origin from reading as rest; 200ms of a steady
  // offset after that is rest.
  const waitForSpineAtRest = (page: import("@playwright/test").Page) =>
    page.evaluate(
      () =>
        new Promise<void>((resolve) => {
          const spine = document.querySelector(".spine") as HTMLElement;
          const start = performance.now();
          let last = spine.scrollLeft;
          let lastChange = start;
          const tick = (): void => {
            const now = performance.now();
            if (spine.scrollLeft !== last) {
              last = spine.scrollLeft;
              lastChange = now;
            }
            if (now - start >= 700 && now - lastChange >= 200) resolve();
            else requestAnimationFrame(tick);
          };
          requestAnimationFrame(tick);
        }),
    );

  test("keeps a reachable scroll range so the frontmost note reveals", async ({
    page,
  }) => {
    await page.goto(TRAIL);
    await expect(page.getByRole("heading", { name: "The Frontmost Note" })).toBeVisible();

    await waitForSpineAtRest(page);
    expect(await frontmostVisibleWidth(page)).toBeGreaterThan(400);
  });

  test("live pinning slides the spine so each new column lands in view", async ({
    page,
  }) => {
    await mountApi(page, LINKED_WORLD);
    await page.goto("/?note=file:one.org");
    await expect(page.getByRole("heading", { name: "Note One" })).toBeVisible();

    for (const next of ["Note Two", "Note Three", "The Frontmost Note"]) {
      await page.locator(".spine-column").last().getByRole("link").first().click();
      await expect(page.getByRole("heading", { name: next })).toBeVisible();
      await waitForSpineAtRest(page);
      expect(await frontmostVisibleWidth(page)).toBeGreaterThan(400);
    }

    expect(await page.locator(".spine-column").count()).toBe(4);
  });
});

test.describe("spine snapping", () => {
  const TRAIL =
    "/?note=file:one.org&stacked=file:two.org&stacked=file:three.org&stacked=file:four.org";

  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("the spine snaps by proximity, one mark per column pin", async ({ page }) => {
    await page.goto(TRAIL);
    await expect(page.getByRole("heading", { name: "The Frontmost Note" })).toBeVisible();

    const declared = await page.evaluate(() => {
      const columns = Array.from(
        document.querySelectorAll(".spine-column"),
      ) as HTMLElement[];
      const marks = Array.from(document.querySelectorAll(".spine-snap")) as HTMLElement[];
      return {
        type: getComputedStyle(document.querySelector(".spine") as HTMLElement)
          .scrollSnapType,
        marks: marks.map((mark) => ({
          align: getComputedStyle(mark).scrollSnapAlign,
          margin: getComputedStyle(mark).scrollMarginLeft,
          width: mark.getBoundingClientRect().width,
        })),
        columnAlign: columns.map((column) => getComputedStyle(column).scrollSnapAlign),
        pins: columns.map((column) => column.style.left),
      };
    });

    expect(declared.type).toBe("x");
    expect(declared.marks.map((mark) => mark.align)).toEqual([
      "start",
      "start",
      "start",
      "start",
    ]);
    expect(declared.marks.map((mark) => mark.width)).toEqual([0, 0, 0, 0]);
    expect(declared.marks.map((mark) => mark.margin)).toEqual(declared.pins);
    expect(declared.columnAlign).toEqual(["none", "none", "none", "none"]);
  });

  test("a free scroll settles with a column at its resting offset", async ({ page }) => {
    await page.goto(TRAIL);
    await expect(page.getByRole("heading", { name: "The Frontmost Note" })).toBeVisible();

    const resting = await restingOffsets(page);
    await page.evaluate(() => {
      (document.querySelector(".spine") as HTMLElement).scrollLeft = 0;
    });

    await page.mouse.move(600, 500);
    await page.mouse.wheel(300, 0);
    await expect
      .poll(async () => {
        const left = await spineScrollLeft(page);
        return left > 0 && resting.includes(left);
      })
      .toBe(true);
  });

  test("a programmatic reveal lands on the target column's own resting offset", async ({
    page,
  }) => {
    await mountApi(page, DEEP_WORLD);
    await page.goto(DEEP_TRAIL);
    await expect(page.getByRole("heading", { name: "Deep Note 31" })).toBeVisible();

    const resting = await restingOffsets(page);
    await page.locator("button.reading-note--obscured").nth(5).click();
    await expect.poll(() => spineScrollLeft(page)).toBe(resting[5]);

    await expect(page.getByRole("heading", { name: "Deep Note 5" })).toBeVisible();
    const revealed = await page.evaluate(() => {
      const spine = document.querySelector(".spine") as HTMLElement;
      const column = document.querySelectorAll(".spine-column")[5] as HTMLElement;
      return {
        left: Math.round(
          column.getBoundingClientRect().left - spine.getBoundingClientRect().left,
        ),
        pin: Math.round(Number.parseFloat(column.style.left || "0")),
      };
    });
    expect(revealed.left).toBe(revealed.pin);
  });
});

test.describe("spine snapping in the narrow layout", () => {
  test.use({ viewport: { width: 375, height: 720 } });

  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("a scroll settling near a seam settles on it", async ({ page }) => {
    await page.goto("/?note=file:one.org&stacked=file:two.org");
    await expect(page.getByRole("heading", { name: "Note Two" })).toBeVisible();

    const run = await page.evaluate(() => {
      const spine = document.querySelector(".spine") as HTMLElement;
      const second = document.querySelectorAll(".spine-column")[1] as HTMLElement;
      return {
        seam: Math.round(
          second.getBoundingClientRect().top -
            spine.getBoundingClientRect().top +
            spine.scrollTop,
        ),
        height: spine.clientHeight,
      };
    });

    await page.evaluate((top) => {
      (document.querySelector(".spine") as HTMLElement).scrollTop = top;
    }, run.seam - Math.round(run.height * 0.2));

    await expect
      .poll(() =>
        page.evaluate(() =>
          Math.round((document.querySelector(".spine") as HTMLElement).scrollTop),
        ),
      )
      .toBe(run.seam);
  });
});
