/*
 * The line under a note's title, stating where the note stands in the filing
 * order. The count rides in on the context the column fetches anyway, so what is
 * asserted here is the painted line, the space it takes from the prose, and that
 * it stays a statement rather than becoming a control.
 */

import { expect, test, type Page } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

/** Three notes in three files, so the order is settled by path alone. */
const WORLD: FixtureWorld = {
  notes: [
    { key: "file:alpha.org", title: "Alpha", body: "The note filed first." },
    { key: "file:beta.org", title: "Beta", body: "The note filed between." },
    { key: "file:gamma.org", title: "Gamma", body: "The note filed last." },
  ],
};

/** The line box the label register sets, and the header's space below it. */
const LINE_HEIGHT = 20;
const HEADER_GAP = 24;

/**
 * What opening a note costs whatever it states about its place: liveness for the
 * shell, the reference resolved to a note for the document title, and the note's
 * own context.
 */
const OPENING_READS = ["/api/status", "/api/node", "/api/note/context"];

/** Parse an `rgb(r, g, b)` string into its channels. */
function channels(color: string): number[] {
  return [...color.matchAll(/\d+/g)].slice(0, 3).map((match) => Number(match[0]));
}

/** Perceived lightness of a painted color, 0 (black) to 255 (white). */
function lightness(color: string): number {
  const [r = 0, g = 0, b = 0] = channels(color);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The size, weight, and painted color of one element's type. */
function typeOf(page: Page, selector: string) {
  return page.locator(selector).evaluate((node) => {
    const style = getComputedStyle(node);
    return {
      size: Number.parseFloat(style.fontSize),
      weight: style.fontWeight,
      color: style.color,
    };
  });
}

test.describe("a note's place in the filing order", () => {
  test("states the position and the size of the collection", async ({ page }) => {
    await mountApi(page, WORLD);
    const reads: string[] = [];
    page.on("request", (request) => {
      const path = new URL(request.url()).pathname;
      if (path.startsWith("/api/")) {
        reads.push(path);
      }
    });

    await page.goto("/?note=file:beta.org");
    await expect(page.getByRole("heading", { name: "Beta" })).toBeVisible();
    await expect(page.locator(".reading-note__place")).toHaveText("Filed 2 of 3");

    // The position rode in on the note's own context read, and nothing beyond the
    // reads that open a note was asked for.
    expect(reads.filter((path) => path === "/api/note/context")).toHaveLength(1);
    expect(reads.filter((path) => !OPENING_READS.includes(path))).toEqual([]);
  });

  test("holds one line at every width, displacing the prose by that line", async ({
    page,
  }) => {
    await mountApi(page, WORLD);
    await page.goto("/?note=file:beta.org");
    const line = page.locator(".reading-note__place");
    await expect(line).toHaveText("Filed 2 of 3");

    // Both layouts: the wide spine of columns, and the narrow single reader.
    for (const width of [1200, 375]) {
      await page.setViewportSize({ width, height: 900 });
      const title = (await page.locator(".reading-note__title").boundingBox())!;
      const box = (await line.boundingBox())!;
      const prose = (await page.locator(".org-paragraph").first().boundingBox())!;

      expect(box.height).toBeLessThanOrEqual(LINE_HEIGHT);
      expect(box.y).toBeGreaterThanOrEqual(title.y + title.height);
      // The prose moved down by the line and by nothing else: the space between
      // the head and the body is the one the header already held.
      expect(prose.y - (title.y + title.height)).toBeLessThanOrEqual(
        box.height + HEADER_GAP,
      );
    }
  });

  test("stays a statement: no stop, no affordance, no hit area of its own", async ({
    page,
  }) => {
    await mountApi(page, WORLD);
    await page.goto("/?note=file:beta.org");
    const line = page.locator(".reading-note__place");
    await expect(line).toHaveText("Filed 2 of 3");

    expect(await line.evaluate((node) => node.tagName)).toBe("P");
    // Nothing takes focus, so nothing stands in the tab order either.
    expect(
      await line.evaluate((node) => {
        node.focus();
        return document.activeElement === node;
      }),
    ).toBe(false);
    // The pointer keeps the cursor it carries over prose.
    expect(await line.evaluate((node) => getComputedStyle(node).cursor)).toBe(
      "auto",
    );
  });

  test("keeps the label register in both schemes, under title and prose", async ({
    page,
  }) => {
    await mountApi(page, WORLD);
    await page.goto("/?note=file:beta.org");
    await expect(page.locator(".reading-note__place")).toHaveText("Filed 2 of 3");
    // The control cycles Auto -> Light -> Dark, so each click pins the next one.
    const control = page.getByRole("button", { name: /^Color scheme:/ });

    for (const scheme of ["light", "dark"]) {
      await control.click();
      await expect(page.locator("html")).toHaveAttribute("data-theme", scheme);

      const place = await typeOf(page, ".reading-note__place");
      const title = await typeOf(page, ".reading-note__title");
      const prose = await typeOf(page, ".org-paragraph");
      const paper = await page.evaluate(
        () => getComputedStyle(document.body).backgroundColor,
      );

      expect(place.size).toBeLessThan(prose.size);
      expect(place.size).toBeLessThan(title.size);
      expect(place.weight).toBe("500");
      // Subordinate in either scheme without naming a tone: the line sits nearer
      // the canvas than the prose does, whichever of the two is the lighter.
      expect(
        Math.abs(lightness(place.color) - lightness(paper)),
      ).toBeLessThan(Math.abs(lightness(prose.color) - lightness(paper)));
    }
  });
});
