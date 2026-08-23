/*
 * Scroll containment on the reading and glossary scrollports.
 *
 * A gesture continued past a scrollport's edge is handed to the browser as a
 * history gesture, which no engine synthesizes under automation. What a real
 * browser does give is the computed `overscroll-behavior` deciding it, plus a
 * wheel proving the scroll range the rule still has to leave reachable; jsdom
 * gives neither.
 */

import { expect, test, type Page } from "@playwright/test";

import { mountApi, tallBody, type FixtureWorld } from "./fixtures.js";

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
    {
      key: "file:entropy.org",
      title: "Entropy",
      body: "A measure of uncertainty.",
      glossaryStatus: "confirmed",
    },
  ],
};

/** Four 625px columns, wider than any frame the specs run in. */
const TRAIL =
  "/?note=file:one.org&stacked=file:two.org&stacked=file:three.org&stacked=file:four.org";

/** Both axes of one element's containment, as the cascade resolves them. */
const containmentOf = (page: Page, selector: string) =>
  page.evaluate((target) => {
    const style = getComputedStyle(document.querySelector(target) as HTMLElement);
    return { x: style.overscrollBehaviorX, y: style.overscrollBehaviorY };
  }, selector);

/** Repeat a wheel gesture until it is well past whatever edge it is aimed at. */
async function wheelPastTheEdge(
  page: Page,
  deltaX: number,
  deltaY: number,
): Promise<void> {
  for (let step = 0; step < 12; step += 1) {
    await page.mouse.wheel(deltaX, deltaY);
  }
}

const spineScroll = (page: Page) =>
  page.evaluate(() => {
    const spine = document.querySelector(".spine") as HTMLElement;
    return {
      left: Math.round(spine.scrollLeft),
      reachX: Math.round(spine.scrollWidth - spine.clientWidth),
      top: Math.round(spine.scrollTop),
      windowScrollY: Math.round(window.scrollY),
    };
  });

test.describe("scroll containment", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("the spine keeps a horizontal overscroll and still reaches both ends", async ({
    page,
  }) => {
    await page.goto(TRAIL);
    await expect(page.getByRole("heading", { name: "The Frontmost Note" })).toBeVisible();
    const address = page.url();

    expect((await containmentOf(page, ".spine")).x).toBe("contain");

    // Aimed at the middle of the frame, so the gesture starts over a column and
    // has to reach the spine through it.
    await page.mouse.move(600, 500);
    await wheelPastTheEdge(page, 400, 0);
    await expect
      .poll(async () => {
        const scroll = await spineScroll(page);
        return scroll.left >= scroll.reachX;
      })
      .toBe(true);
    expect(page.url()).toBe(address);

    await wheelPastTheEdge(page, -400, 0);
    await expect.poll(async () => (await spineScroll(page)).left).toBe(0);
    expect(page.url()).toBe(address);
  });

  test("a column keeps its own pull and leaves the swipe to the spine", async ({
    page,
  }) => {
    await page.goto("/?note=file:one.org");
    await expect(page.getByRole("heading", { name: "Note One" })).toBeVisible();
    const address = page.url();

    const column = await containmentOf(page, ".spine-column");
    expect(column.y).toBe("contain");
    // The spine is the horizontal scroller under every column, and a column
    // containing that axis too would take the swipe away from it.
    expect(column.x).toBe("auto");

    await page.mouse.move(600, 500);
    await wheelPastTheEdge(page, 0, 400);
    await expect
      .poll(async () =>
        page.evaluate(() => {
          const note = document.querySelector(".spine-column") as HTMLElement;
          return Math.round(note.scrollTop) >= note.scrollHeight - note.clientHeight;
        }),
      )
      .toBe(true);
    expect((await spineScroll(page)).windowScrollY).toBe(0);
    expect(page.url()).toBe(address);
  });

  test("the page's own scroll is left to the browser", async ({ page }) => {
    await page.goto("/?note=file:one.org");
    await expect(page.getByRole("heading", { name: "Note One" })).toBeVisible();

    // Containment on the viewport would also take a pull-to-refresh, which is
    // the browser's to offer.
    for (const selector of [":root", "body"]) {
      expect(await containmentOf(page, selector)).toEqual({ x: "auto", y: "auto" });
    }
  });

  test("the glossary's list and pane keep their own overscroll", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText("A measure of uncertainty.")).toBeVisible();

    for (const selector of [".glossary-terms", ".glossary-detail"]) {
      expect(await containmentOf(page, selector)).toEqual({
        x: "contain",
        y: "contain",
      });
    }
  });
});

/*
 * Below the 800px breakpoint the spine is a vertical scroller inside the page
 * and the columns stack inside it, so containment is a different statement on
 * different boxes.
 */
test.describe("scroll containment in the narrow layout", () => {
  test.use({ viewport: { width: 375, height: 720 } });

  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("the stacked spine keeps its own pull and still scrolls", async ({ page }) => {
    await page.goto("/?note=file:one.org");
    await expect(page.getByRole("heading", { name: "Note One" })).toBeVisible();
    const address = page.url();

    expect(await containmentOf(page, ".spine")).toEqual({
      x: "contain",
      y: "contain",
    });
    // A stacked column scrolls nothing of its own here, so its containment is
    // released: latched on the axis the spine now scrolls, it would hold the run
    // still.
    expect((await containmentOf(page, ".spine-column")).y).toBe("auto");

    await page.mouse.move(180, 400);
    await wheelPastTheEdge(page, 0, 400);
    await expect.poll(async () => (await spineScroll(page)).top > 0).toBe(true);
    expect((await spineScroll(page)).windowScrollY).toBe(0);
    expect(page.url()).toBe(address);
  });

  test("the narrow glossary surface keeps its own pull", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText("A measure of uncertainty.")).toBeVisible();

    expect((await containmentOf(page, ".glossary")).y).toBe("contain");
  });
});
