import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

const WORLD: FixtureWorld = {
  notes: [
    {
      key: "file:alpha.org",
      id: "alpha-id",
      title: "Alpha",
      body: "Alpha links to [[id:beta-id][Beta]].",
      glossaryStatus: "confirmed",
      srDue: "2026-08-01",
    },
    {
      key: "file:beta.org",
      id: "beta-id",
      title: "Beta",
      body: "Beta defines the second term.",
      glossaryStatus: "confirmed",
    },
  ],
};

async function expectAccessible(page: Page, surface: string): Promise<void> {
  for (const theme of ["light", "dark"]) {
    await page.locator("html").evaluate((element, value) => {
      element.setAttribute("data-theme", value);
    }, theme);
    const result = await new AxeBuilder({ page }).analyze();
    expect(
      result.violations.map(({ id, impact, nodes }) => ({
        id,
        impact,
        targets: nodes.map((node) => node.target),
      })),
      `${surface} (${theme})`,
    ).toEqual([]);
  }
}

test.describe("accessibility audit", () => {
  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("covers every desktop surface", async ({ page }) => {
    await page.goto("/");
    await expect(
      page.getByRole("combobox", { name: "Search the slipbox" }),
    ).toBeVisible();
    await expectAccessible(page, "notes entry");

    await page.getByRole("combobox", { name: "Search the slipbox" }).fill("Alpha");
    await expect(page.getByText("1 note matches.")).toBeVisible();
    await expectAccessible(page, "notes results");

    await page.getByRole("button", { name: "Glossary" }).click();
    await expect(page.getByRole("listbox", { name: "Glossary terms" })).toBeVisible();
    await expectAccessible(page, "glossary browse");

    await page.getByRole("button", { name: "Due terms" }).click();
    await expect(page.getByText(/^1 term is due,/)).toBeVisible();
    await expectAccessible(page, "glossary due");

    await page.goto("/?note=file:alpha.org");
    await expect(page.getByRole("heading", { name: "Alpha" })).toBeVisible();
    await expectAccessible(page, "reader");

    await page.goto("/?note=file:missing-note.org");
    await expect(page.getByText("This note is not in the slipbox.")).toBeVisible();
    await expectAccessible(page, "reader error");
  });
});

test.describe("mobile accessibility audit", () => {
  test.use({ viewport: { width: 375, height: 720 }, hasTouch: true });

  test.beforeEach(async ({ page }) => {
    await mountApi(page, WORLD);
  });

  test("covers entry, glossary, reader, and error states", async ({ page }) => {
    await page.goto("/");
    await expectAccessible(page, "mobile notes entry");

    await page.getByRole("combobox", { name: "Search the slipbox" }).fill("Alpha");
    await expect(page.getByText("1 note matches.")).toBeVisible();
    await expectAccessible(page, "mobile notes results");

    await page.getByRole("button", { name: "Glossary" }).tap();
    await expect(page.getByRole("listbox", { name: "Glossary terms" })).toBeVisible();
    await expectAccessible(page, "mobile glossary list");

    await page.getByRole("option", { name: "Alpha" }).tap();
    await expect(page.getByRole("button", { name: "Back to terms" })).toBeVisible();
    await expectAccessible(page, "mobile glossary detail");

    await page.goto("/?note=file:alpha.org");
    await expect(page.getByRole("heading", { name: "Alpha" })).toBeVisible();
    await expectAccessible(page, "mobile reader");

    await page.goto("/?note=file:missing-note.org");
    await expect(page.getByText("This note is not in the slipbox.")).toBeVisible();
    await expectAccessible(page, "mobile reader error");
  });
});
