
import { expect, test } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

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
      mentions: [
        {
          source: { key: "file:naming.org", id: "naming-uuid", title: "Naming Note" },
          line: `${"padding words ".repeat(12)}Origin is named without a link`,
          matched: "Origin",
        },
      ],
    },
    {
      key: "file:dense.org",
      title: "Dense",
      body: "Six notes cite this one.",
      backlinks: [
        { key: "file:c1.org", id: "c1-uuid", title: "First", preview: "cites Dense" },
        { key: "file:c2.org", id: "c2-uuid", title: "Second", preview: "cites Dense" },
        { key: "file:c3.org", id: "c3-uuid", title: "Third", preview: "cites Dense" },
        { key: "file:c4.org", id: "c4-uuid", title: "Fourth", preview: "cites Dense" },
        { key: "file:c5.org", id: "c5-uuid", title: "Fifth", preview: "cites Dense" },
        { key: "file:c6.org", id: "c6-uuid", title: "Sixth", preview: "cites Dense" },
      ],
    },
  ],
};

let explored: string[] = [];
let scanned: string[] = [];

test.describe("the relations footer", () => {
  test.beforeEach(async ({ page }) => {
    explored = [];
    scanned = [];
    page.on("request", (request) => {
      const url = new URL(request.url());
      if (url.pathname === "/api/explore") {
        explored.push(url.search);
      }
      if (url.pathname === "/api/unlinked-references") {
        scanned.push(url.search);
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

  test("keeps the scanned match inside the line the row paints", async ({ page }) => {
    const group = page.getByRole("button", { name: "Unlinked mentions" });
    await expect(group).toHaveAttribute("aria-expanded", "false");
    expect(scanned).toHaveLength(0);

    await group.click();
    await expect(page.getByRole("link", { name: "Naming Note" })).toBeVisible();
    expect(scanned).toHaveLength(1);

    const mark = page.locator("mark.relations__match");
    await expect(mark).toHaveText("Origin");
    const preview = page.locator(".relations__preview", { has: mark });
    await expect(preview).toHaveText(/^…/);

    const marked = await mark.boundingBox();
    const line = await preview.boundingBox();
    expect(marked!.x).toBeGreaterThanOrEqual(line!.x);
    expect(marked!.x + marked!.width).toBeLessThanOrEqual(line!.x + line!.width);
  });
});

const INVENTORY_ROW = 24;

const INVENTORY_FOOTER = 260;

const REGISTER_GAP = 24;

test.describe("the footer as an inventory", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
    await page.goto("/?note=file:dense.org");
    await expect(page.getByRole("heading", { name: "Dense" })).toBeVisible();
  });

  test("sets six related notes at an index's density, not the prose's", async ({
    page,
  }) => {
    const rows = await page.locator(".relations__row").all();
    expect(rows).toHaveLength(6);
    for (const row of rows) {
      expect((await row.boundingBox())!.height).toBeLessThanOrEqual(INVENTORY_ROW);
    }

    const footer = (await page.locator(".relations").boundingBox())!;
    expect(footer.height).toBeLessThanOrEqual(INVENTORY_FOOTER);

    const readOn = page.locator(".read-on");
    const crossed =
      (await readOn.count()) > 0 ? readOn : page.locator(".org-paragraph").last();
    const above = (await crossed.boundingBox())!;
    expect(footer.y - (above.y + above.height)).toBeGreaterThanOrEqual(REGISTER_GAP);
    const rule = await page
      .locator(".relations")
      .evaluate((node) => getComputedStyle(node).borderTopWidth);
    expect(rule).toBe("1px");

    await page.locator(".relations").evaluate((node) => {
      node.style.marginTop = "0px";
    });
    const joined = (await page.locator(".relations").boundingBox())!;
    expect(joined.y - (above.y + above.height)).toBeLessThan(REGISTER_GAP);
  });

  test("gives every group one label register, whenever the group was added", async ({
    page,
  }) => {
    const labels = page.locator(".relations__label, .relations__toggle");
    await expect(labels).toHaveCount(5);

    for (const name of ["Related notes", "Unlinked mentions"]) {
      const group = page.getByRole("button", { name });
      await group.click();
      await expect(group).toHaveAttribute("aria-expanded", "true");
    }
    await expect(labels).toHaveCount(5);

    const register = await labels.evaluateAll((nodes) =>
      nodes.map((node) => {
        const style = getComputedStyle(node);
        return [
          style.fontSize,
          style.lineHeight,
          style.fontWeight,
          style.textTransform,
          style.letterSpacing,
          style.color,
        ].join(" ");
      }),
    );
    expect(new Set(register).size).toBe(1);

    const transforms = await labels.evaluateAll((nodes) =>
      nodes.map((node) => getComputedStyle(node).textTransform),
    );
    expect(new Set(transforms)).toEqual(new Set(["none"]));

    const label = await labels
      .first()
      .evaluate((node) => parseFloat(getComputedStyle(node).fontSize));
    const prose = await page
      .locator(".org-document")
      .evaluate((node) => parseFloat(getComputedStyle(node).fontSize));
    expect(label).toBeLessThan(prose);
  });
});
