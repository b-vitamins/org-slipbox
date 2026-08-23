/*
 * The relations footer's own layout. A hanging mark is placed by the cascade
 * against the row it hangs off, so only a real browser reports where it and the
 * title it precedes actually sit.
 */

import { expect, test } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

/** One note carrying a row of each direction: out only, both ways, in only. */
const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:origin.org",
      title: "Origin",
      body: "Links out to [[id:out-uuid][the outbound note]].",
      forwardLinks: [
        { key: "file:out.org", id: "out-uuid", title: "Outbound", preview: "links out" },
        { key: "file:both.org", id: "both-uuid", title: "Reciprocal", preview: "links out" },
      ],
      backlinks: [
        {
          key: "file:both.org",
          id: "both-uuid",
          title: "Reciprocal",
          preview: "cites the origin back",
        },
        { key: "file:in.org", id: "in-uuid", title: "Inbound", preview: "cites the origin" },
      ],
    },
  ],
};

test.describe("the relations footer", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
    await page.goto("/?note=file:origin.org");
    await expect(page.getByRole("heading", { name: "Origin" })).toBeVisible();
  });

  test("hangs each direction mark off a single column of titles", async ({ page }) => {
    const titles = await page.locator("a.relations__link").all();
    expect(titles).toHaveLength(3);

    const lefts = new Set<number>();
    for (const title of titles) {
      const box = await title.boundingBox();
      lefts.add(Math.round(box!.x));
    }
    expect(lefts.size).toBe(1);

    // The mark hangs to the left of that column rather than indenting the title
    // it belongs to.
    const mark = await page.locator(".relations__direction").first().boundingBox();
    expect(mark!.x + mark!.width).toBeLessThanOrEqual([...lefts][0]!);
  });
});
