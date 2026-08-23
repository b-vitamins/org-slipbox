import { expect, test } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:source.org",
      title: "Source Note",
      body:
        "See [[id:target-uuid][the target]] for the rest.\n\n" +
        "A plain line of prose, standing here with no link in it.\n\n" +
        "Out to [[https://example.org][the web]], or to " +
        "[[file:elsewhere.org][a local file]].",
      forwardLinks: [
        {
          key: "file:target.org",
          id: "target-uuid",
          title: "Target Note",
          preview: "the rest is here",
        },
      ],
    },
    {
      key: "file:target.org",
      id: "target-uuid",
      title: "Target Note",
      body: "The target of the link.",
    },
  ],
};

test.describe("native links", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("an in-body link's href is the router's own note URL, not a fake hash", async ({
    page,
  }) => {
    await page.goto("/?note=file:source.org");
    const link = page.getByRole("link", { name: "the target" });
    await expect(link).toBeVisible();

    const href = await link.getAttribute("href");
    // The encoded stack that opens the link's target as a fresh root.
    expect(href).toBe("?note=id%3Atarget-uuid");
    expect(href).not.toContain("#/");
  });

  test("loading a link's href cold opens the target note", async ({ page }) => {
    await page.goto("/?note=file:source.org");
    const href = await page
      .getByRole("link", { name: "the target" })
      .getAttribute("href");
    expect(href).toBeTruthy();

    await page.goto(`/${href}`);
    await expect(page.getByRole("heading", { name: "Target Note" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Source Note" })).toHaveCount(0);
  });

  test("marks an in-prose link with a cue that is not its color", async ({ page }) => {
    await page.goto("/?note=file:source.org");
    const link = page.getByRole("link", { name: "the target" });
    await expect(link).toBeVisible();

    // Read with the pointer elsewhere, so what is asserted is the resting state
    // rather than the hover the surface has always underlined.
    const decoration = await link.evaluate((node) => {
      const style = getComputedStyle(node);
      return { line: style.textDecorationLine, style: style.textDecorationStyle };
    });
    expect(decoration.line).toBe("underline");
    expect(decoration.style).toBe("solid");
  });

  test("carries the cue without moving the line it sits in", async ({ page }) => {
    await page.goto("/?note=file:source.org");
    await expect(page.getByRole("link", { name: "the target" })).toBeVisible();

    // One line each, the same prose scale: the paragraph holding a link stands as
    // tall as the one holding none, which is what a decoration inside the line box
    // leaves untouched.
    const heights = await page
      .locator(".org-document .org-paragraph")
      .evaluateAll((nodes) => nodes.map((node) => node.getBoundingClientRect().height));
    expect(heights[0]).toBe(heights[1]);
  });

  test("marks an unfollowed target and an external one each their own way", async ({
    page,
  }) => {
    await page.goto("/?note=file:source.org");

    // Neither variant takes the underline above: an inert label keeps its dotted
    // one, and an external link keeps its trailing marker, so no cue is doubled.
    const inert = page.locator(".org-link--inert", { hasText: "a local file" });
    await expect(inert).toBeVisible();
    expect(
      await inert.evaluate((node) => getComputedStyle(node).textDecorationStyle),
    ).toBe("dotted");

    const external = page.getByRole("link", { name: "the web" });
    const marker = await external.evaluate((node) => ({
      line: getComputedStyle(node).textDecorationLine,
      after: getComputedStyle(node, "::after").content,
    }));
    expect(marker.line).toBe("none");
    expect(marker.after).toContain("↗");
  });

  test("a relation-row link carries the same real destination", async ({ page }) => {
    await page.goto("/?note=file:source.org");
    const relation = page.locator("a.relations__link", { hasText: "Target Note" });
    await expect(relation).toBeVisible();
    expect(await relation.getAttribute("href")).toBe("?note=id%3Atarget-uuid");
  });
});
