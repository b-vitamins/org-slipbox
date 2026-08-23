
import { expect, test } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:entropy.org",
      title: "Entropy",
      body: "A measure of uncertainty, dual to [[id:prior-uuid][the prior]].",
      glossaryStatus: "confirmed",
    },
    {
      key: "file:prior.org",
      id: "prior-uuid",
      title: "Prior",
      body: "What the model believes before it sees the data.",
    },
  ],
};

const DEFINITION = "A measure of uncertainty, dual to";

const PAGE = 25;
const LONG: FixtureWorld = {
  notes: Array.from({ length: 30 }, (_, index) => ({
    key: `file:term-${index + 1}.org`,
    title: `Term ${index + 1}`,
    body: `Definition ${index + 1}.`,
    glossaryStatus: "confirmed" as const,
  })),
};

const DUE_LONG: FixtureWorld = {
  notes: Array.from({ length: 30 }, (_, index) => ({
    key: `file:due-${index + 1}.org`,
    title: index === 27 ? "Needle term" : `Due term ${index + 1}`,
    body: `Definition ${index + 1}.`,
    glossaryStatus: "confirmed" as const,
    srDue: "2026-01-01",
  })),
};

const TOUCH_TARGET = 44;

test.describe("the glossary peek", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("a link inside a definition opens the note it names", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    await page.getByRole("link", { name: "the prior" }).click();

    await expect(page.getByRole("heading", { name: "Prior" })).toBeVisible();
    await expect(page.getByText("What the model believes")).toBeVisible();
    await expect(page.locator(".glossary")).toHaveCount(0);
  });

  test("resting on a link in a definition commits nothing", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    await page.getByRole("link", { name: "the prior" }).hover();

    await expect(page.locator(".glossary")).toBeVisible();
    await expect(page.locator(".glance-card")).toHaveCount(0);
  });

  test("the term's own control opens it in the reader", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    const control = page.getByRole("link", { name: "Open in reader" });
    expect(await control.getAttribute("href")).toBe("?note=file%3Aentropy.org");

    await control.click();

    await expect(
      page.getByRole("heading", { name: "Entropy", level: 1 }),
    ).toBeVisible();
    await expect(page.locator(".glossary")).toHaveCount(0);
  });

  test("the opened note is a history entry the way back undoes", async ({ page }) => {
    await page.goto("/?view=glossary");
    await page.getByRole("link", { name: "the prior" }).click();
    await expect(page.getByRole("heading", { name: "Prior" })).toBeVisible();

    await page.goBack();
    await expect(page.getByText(DEFINITION)).toBeVisible();
  });
});

test.describe("a glossary longer than one page", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, LONG);
  });

  test("browses to its last term past the cut it states", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByRole("option", { name: "Term 25" })).toBeVisible();

    await expect(
      page.getByText(`${PAGE} of ${LONG.notes.length} terms read.`),
    ).toBeVisible();
    await expect(page.getByRole("option", { name: "Term 26" })).toHaveCount(0);

    await page.getByRole("button", { name: "Read more terms" }).click();

    await expect(page.getByRole("option", { name: "Term 30" })).toBeVisible();
    await expect(page.getByRole("option")).toHaveCount(30);
    await expect(page.getByText("terms read.")).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Read more terms" }),
    ).toHaveCount(0);
  });

  test("continues the list from the end of the list's own scrollport", async ({
    page,
  }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByRole("option", { name: "Term 25" })).toBeVisible();

    const list = page.locator(".glossary-terms");
    expect(
      await list.evaluate((box) => box.scrollHeight - box.clientHeight),
    ).toBeGreaterThan(0);

    await list.evaluate((box) => box.scrollTo(0, box.scrollHeight));

    await expect(page.getByRole("option", { name: "Term 30" })).toBeVisible();
    await expect(page.getByRole("option")).toHaveCount(LONG.notes.length);
  });

  test("holds the peeked term across the page that follows it", async ({ page }) => {
    await page.goto("/?view=glossary");
    const marked = page.getByRole("option", { name: "Term 3", exact: true });
    const field = page.getByRole("combobox");
    await field.press("ArrowDown");
    await field.press("ArrowDown");
    await expect(marked).toHaveAttribute("aria-selected", "true");

    await page
      .locator(".glossary-terms")
      .evaluate((box) => box.scrollTo(0, box.scrollHeight));
    await expect(page.getByRole("option", { name: "Term 30" })).toBeVisible();

    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(
      page.getByRole("heading", { name: "Term 3", level: 2, exact: true }),
    ).toBeVisible();
  });

  test("takes a hover from the pointer's own move, not from rows moving under it", async ({
    page,
  }) => {
    await page.goto("/?view=glossary");
    const third = page.getByRole("option", { name: "Term 3", exact: true });
    await expect(third).toBeVisible();

    await third.hover();
    await expect(third).toHaveAttribute("aria-selected", "true");
    expect(new URL(page.url()).searchParams.get("term")).toBe("file:term-3.org");

    await page.getByRole("combobox").press("q");
    await expect(page.getByText("Searching needs a word")).toBeVisible();

    await expect(third).toHaveAttribute("aria-selected", "true");
    expect(new URL(page.url()).searchParams.get("term")).toBe("file:term-3.org");

    const fifth = page.getByRole("option", { name: "Term 5", exact: true });
    await fifth.hover();
    await expect(fifth).toHaveAttribute("aria-selected", "true");
    expect(new URL(page.url()).searchParams.get("term")).toBe("file:term-5.org");
  });
});

test.describe("a due list longer than one page", () => {
  test("search reaches a match beyond the first unfiltered page", async ({ page }) => {
    await mountApi(page, DUE_LONG);
    await page.goto("/?view=review");
    await expect(page.getByRole("option", { name: /^Due term 25/ })).toBeVisible();
    await expect(page.getByRole("option", { name: "Needle term" })).toHaveCount(0);

    await page.getByRole("combobox", { name: "Search due terms" }).fill("needle");

    await expect(page.getByRole("option", { name: /^Needle term/ })).toBeVisible();
    await expect(page.getByText("1 due term matches, in schedule order.")).toBeVisible();
  });
});

test.describe("the glossary term in the address", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, LONG);
  });

  test("a copied address reopens the definition it names", async ({ page }) => {
    await page.goto("/?view=glossary&term=file%3Aterm-3.org");

    const marked = page.getByRole("option", { name: "Term 3", exact: true });
    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(
      page.getByRole("heading", { name: "Term 3", level: 2, exact: true }),
    ).toBeVisible();
    await expect(page.getByText("Definition 3.")).toBeVisible();
  });

  test("a reload holds the open term, which cost no history entry", async ({
    page,
  }) => {
    await page.goto("/?view=glossary");
    const marked = page.getByRole("option", { name: "Term 3", exact: true });
    const entries = await page.evaluate(() => history.length);

    await marked.click();
    await expect(page.getByText("Definition 3.")).toBeVisible();

    expect(new URL(page.url()).search).toBe(
      "?view=glossary&term=file%3Aterm-3.org",
    );
    expect(await page.evaluate(() => history.length)).toBe(entries);

    await page.reload();

    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(page.getByText("Definition 3.")).toBeVisible();
  });

  test("leaves the term on the entry it came from when the surface changes", async ({
    page,
  }) => {
    await page.goto("/?view=glossary");
    const marked = page.getByRole("option", { name: "Term 3", exact: true });
    await marked.click();
    await expect(page.getByText("Definition 3.")).toBeVisible();

    await page.getByRole("button", { name: "Notes" }).click();

    await expect(page.getByRole("combobox", { name: "Search the slipbox" })).toBeVisible();
    expect(new URL(page.url()).search).toBe("");

    await page.goBack();

    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(page.getByText("Definition 3.")).toBeVisible();
  });

  test("an address naming a term past the page read reads it anyway", async ({
    page,
  }) => {
    await page.goto("/?view=glossary&term=file%3Aterm-28.org");

    await expect(page.getByText("Definition 28.")).toBeVisible();
    const marked = page.getByRole("option", { name: "Term 28", exact: true });
    await expect(marked).toBeVisible();
    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(
      page.getByText(
        "Term 28 was opened from its link and is outside the current page.",
      ),
    ).toBeVisible();

    await page
      .locator(".glossary-terms")
      .evaluate((box) => box.scrollTo(0, box.scrollHeight));

    await expect(marked).toHaveAttribute("aria-selected", "true");
    await expect(
      page.getByText("was opened from its link and is outside the current page."),
    ).toHaveCount(0);
  });

  test("an address naming no term says so and leaves the list usable", async ({
    page,
  }) => {
    await page.goto("/?view=glossary&term=file%3Amissing.org");

    await expect(
      page.getByText("No glossary term is named file:missing.org."),
    ).toBeVisible();
    await expect(page.getByText("Select a term to read its definition.")).toBeVisible();

    await page.getByRole("option", { name: "Term 1", exact: true }).click();

    await expect(page.getByText("Definition 1.")).toBeVisible();
    await expect(
      page.getByText("No glossary term is named file:missing.org."),
    ).toHaveCount(0);
  });
});

test.describe("the glossary peek under a hoverless pointer", () => {
  test.use({ hasTouch: true });

  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("a tap on a link in a definition opens it, having no card to commit from", async ({
    page,
  }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    await page.getByRole("link", { name: "the prior" }).tap();

    await expect(page.locator(".glance-card")).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "Prior" })).toBeVisible();
  });

  test("the pane and its term list end at the visible viewport", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    const geometry = await page.evaluate(() => {
      const bottomOf = (selector: string): number =>
        (document.querySelector(selector) as HTMLElement).getBoundingClientRect()
          .bottom;
      return {
        visibleViewport: Math.round(window.visualViewport?.height ?? 0),
        paneBottom: Math.round(bottomOf(".glossary")),
        termsBottom: Math.round(bottomOf(".glossary-terms")),
        pageScrollHeight: document.documentElement.scrollHeight,
      };
    });

    expect(geometry.paneBottom).toBe(geometry.visibleViewport);
    expect(geometry.termsBottom).toBeLessThanOrEqual(geometry.visibleViewport);
    expect(geometry.pageScrollHeight).toBeLessThanOrEqual(geometry.visibleViewport);
  });

  test("the term's own control clears the touch-target floor", async ({ page }) => {
    await page.goto("/?view=glossary");
    await expect(page.getByText(DEFINITION)).toBeVisible();

    const box = await page
      .getByRole("link", { name: "Open in reader" })
      .boundingBox();
    expect(box!.height).toBeGreaterThanOrEqual(TOUCH_TARGET);
  });
});
