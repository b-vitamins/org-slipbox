/*
 * The relations footer's own layout, and the cost of the groups beside its
 * directed inventory. A hanging mark or label is placed by the cascade against
 * the row it hangs off, so only a real browser reports where it and the title it
 * precedes actually sit; and only a real browser reports which requests a column
 * made before a reader touched it.
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
      // Ranked by the lens: the two-connector candidate leads, and the third
      // stands under a connector of its own.
      bridges: [
        {
          key: "file:bridged.org",
          id: "bridged-uuid",
          title: "Bridged Note",
          via: [
            { key: "file:out.org", id: "out-uuid", title: "Outbound" },
            { key: "file:in.org", id: "in-uuid", title: "Inbound" },
          ],
        },
        {
          key: "file:second.org",
          id: "second-uuid",
          title: "Second Bridged",
          via: [{ key: "file:out.org", id: "out-uuid", title: "Outbound" }],
        },
        {
          key: "file:third.org",
          id: "third-uuid",
          title: "Third Bridged",
          via: [{ key: "file:in.org", id: "in-uuid", title: "Inbound" }],
        },
      ],
    },
  ],
};

/** Every `/api/explore` request the page made, in order. */
let explored: string[] = [];

test.describe("the relations footer", () => {
  test.beforeEach(async ({ page }) => {
    explored = [];
    page.on("request", (request) => {
      const url = new URL(request.url());
      if (url.pathname === "/api/explore") {
        explored.push(url.search);
      }
    });
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

  test("asks the lens nothing until the related group is opened", async ({ page }) => {
    const group = page.getByRole("button", { name: "Related notes" });
    await expect(group).toHaveAttribute("aria-expanded", "false");
    expect(explored).toHaveLength(0);

    await group.click();
    await expect(page.getByRole("link", { name: "Bridged Note" })).toBeVisible();

    expect(explored).toHaveLength(1);
    expect(explored[0]).toContain("lens=bridges");
  });

  test("hangs a connector left of the rows it names", async ({ page }) => {
    await page.getByRole("button", { name: "Related notes" }).click();
    await expect(page.getByRole("link", { name: "Bridged Note" })).toBeVisible();

    // Two connectors, each named once, in the order the lens reached them.
    await expect(page.locator(".relations__connector")).toHaveText([
      "via Outbound",
      "via Inbound",
    ]);

    const connector = await page.locator(".relations__connector").first().boundingBox();
    const row = await page
      .getByRole("link", { name: "Bridged Note" })
      .boundingBox();
    expect(connector!.x).toBeLessThan(row!.x);
    expect(connector!.y).toBeLessThan(row!.y);
  });
});
