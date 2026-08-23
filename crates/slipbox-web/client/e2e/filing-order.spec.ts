
import { expect, test, type Locator, type Page } from "@playwright/test";

import { mountApi, type FixtureWorld } from "./fixtures.js";

const WORLD: FixtureWorld = {
  notes: [
    { key: "file:alpha.org", title: "Alpha", body: "The note filed first." },
    { key: "file:beta.org", title: "Beta", body: "The note filed between." },
    { key: "file:gamma.org", title: "Gamma", body: "The note filed last." },
  ],
};

const LINE_HEIGHT = 20;
const HEADER_GAP = 24;

const OPENING_READS = ["/api/status", "/api/node", "/api/note/context"];

function channels(color: string): number[] {
  return [...color.matchAll(/\d+/g)].slice(0, 3).map((match) => Number(match[0]));
}

function lightness(color: string): number {
  const [r = 0, g = 0, b = 0] = channels(color);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

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

    for (const width of [1200, 375]) {
      await page.setViewportSize({ width, height: 900 });
      const title = (await page.locator(".reading-note__title").boundingBox())!;
      const box = (await line.boundingBox())!;
      const prose = (await page.locator(".org-paragraph").first().boundingBox())!;

      expect(box.height).toBeLessThanOrEqual(LINE_HEIGHT);
      expect(box.y).toBeGreaterThanOrEqual(title.y + title.height);
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
    expect(
      await line.evaluate((node) => {
        node.focus();
        return document.activeElement === node;
      }),
    ).toBe(false);
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
      expect(
        Math.abs(lightness(place.color) - lightness(paper)),
      ).toBeLessThan(Math.abs(lightness(prose.color) - lightness(paper)));
    }
  });
});

const WORDY: FixtureWorld = {
  notes: [
    {
      key: "file:alpha.org",
      title: "Alpha, the note standing first in the order",
      body: "The note filed first.",
    },
    {
      key: "file:beta.org",
      title: "Beta, the note standing between the others",
      body: "The note filed between.",
    },
    {
      key: "file:gamma.org",
      title: "Gamma, the note standing last in the order",
      body: "The note filed last.",
    },
  ],
};

function address(note: string, ...stacked: string[]): string {
  const stack = stacked.map((key) => `&stacked=${encodeURIComponent(key)}`);
  return `/?note=${encodeURIComponent(note)}${stack.join("")}`;
}

function isWhole(link: Locator): Promise<boolean> {
  return link.evaluate(
    (node) =>
      node.scrollWidth <= node.clientWidth && node.scrollHeight <= node.clientHeight,
  );
}

function isHit(link: Locator): Promise<boolean> {
  return link.evaluate((node) => {
    const box = node.getBoundingClientRect();
    const at = document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2);
    return at instanceof Element && at.closest(".read-on__link") === node;
  });
}

test.describe("reading on in the filing order", () => {
  test("offers a move to each side, naming the side and the note it reaches", async ({
    page,
  }) => {
    await mountApi(page, WORLD);
    const reads: string[] = [];
    page.on("request", (request) => {
      const path = new URL(request.url()).pathname;
      if (path.startsWith("/api/")) {
        reads.push(path);
      }
    });

    await page.goto("/?note=file:beta.org");
    await expect(page.locator(".read-on__link")).toHaveCount(2);
    await expect(
      page.getByRole("link", { name: /^Filed before this\s*Alpha$/ }),
    ).toBeVisible();
    await expect(
      page.getByRole("link", { name: /^Filed after this\s*Gamma$/ }),
    ).toBeVisible();
    expect(reads.filter((path) => !OPENING_READS.includes(path))).toEqual([]);

    const prose = (await page.locator(".org-paragraph").last().boundingBox())!;
    const pair = (await page.locator(".read-on").boundingBox())!;
    const footer = (await page.locator(".relations").boundingBox())!;
    expect(pair.y).toBeGreaterThanOrEqual(prose.y + prose.height);
    expect(footer.y).toBeGreaterThanOrEqual(pair.y + pair.height);
    expect(await page.locator(".relations .read-on").count()).toBe(0);

    expect(await page.locator(".read-on").locator("img, svg, [role]").count()).toBe(0);
    for (const pseudo of ["::before", "::after"]) {
      expect(
        await page
          .locator(".read-on__link")
          .first()
          .evaluate((node, at) => getComputedStyle(node, at).content, pseudo),
      ).toBe("none");
    }

    const link = await typeOf(page, ".read-on__link:first-child");
    const body = await typeOf(page, ".org-paragraph");
    expect(link.color).not.toBe(body.color);
  });

  test("offers the one move an end of the order holds, and keeps no space for the other", async ({
    page,
  }) => {
    await mountApi(page, WORLD);

    await page.goto("/?note=file:alpha.org");
    await expect(page.locator(".read-on__link")).toHaveCount(1);
    await expect(
      page.getByRole("link", { name: /^Filed after this\s*Beta$/ }),
    ).toBeVisible();
    await expect(page.getByText("Filed before this")).toHaveCount(0);

    const title = (await page.locator(".reading-note__title").boundingBox())!;
    const move = (await page.locator(".read-on__link").boundingBox())!;
    expect(Math.abs(move.x - title.x)).toBeLessThanOrEqual(1);

    await page.goto("/?note=file:gamma.org");
    await expect(page.locator(".read-on__link")).toHaveCount(1);
    await expect(
      page.getByRole("link", { name: /^Filed before this\s*Beta$/ }),
    ).toBeVisible();
  });

  test("replaces the note being read, keeps the trail, and states the result", async ({
    page,
  }) => {
    await mountApi(page, WORLD);
    await page.goto(address("file:alpha.org", "file:beta.org"));
    await expect(page.locator(".reading-note__title")).toHaveText(["Alpha", "Beta"]);

    await page.getByRole("link", { name: /^Filed after this\s*Gamma$/ }).click();

    await expect(page.locator(".reading-note__title")).toHaveText(["Alpha", "Gamma"]);
    await expect(page).toHaveURL(address("file:alpha.org", "file:gamma.org"));

    await page.goBack();
    await expect(page.locator(".reading-note__title")).toHaveText(["Alpha", "Beta"]);
    await expect(page).toHaveURL(address("file:alpha.org", "file:beta.org"));
  });

  test("reveals a neighbor the trail already holds, rather than opening a second", async ({
    page,
  }) => {
    await mountApi(page, WORLD);
    const opened = address("file:alpha.org", "file:beta.org");
    await page.goto(opened);
    await expect(page.locator(".reading-note__title")).toHaveText(["Alpha", "Beta"]);

    await page.getByRole("link", { name: /^Filed before this\s*Alpha$/ }).click();

    await expect(page.locator(".spine-column")).toHaveCount(2);
    await expect(page.locator(".reading-note__title")).toHaveText(["Alpha", "Beta"]);
    await expect(page).toHaveURL(opened);
  });

  test("is reached and followed from the keyboard alone", async ({ page }) => {
    await mountApi(page, WORLD);
    await page.goto("/?note=file:beta.org");
    const later = page.getByRole("link", { name: /^Filed after this\s*Gamma$/ });
    await expect(later).toBeVisible();

    const focused = (): Promise<boolean> =>
      later.evaluate((node) => node === document.activeElement);
    for (let stop = 0; stop < 20 && !(await focused()); stop += 1) {
      await page.keyboard.press("Tab");
    }
    await expect(later).toBeFocused();

    await page.keyboard.press("Enter");
    await expect(page.locator(".reading-note__title")).toHaveText(["Gamma"]);
  });

  test("keeps both moves whole and hit-testable where the pair wraps", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 375, height: 900 });
    await mountApi(page, WORDY);
    await page.goto("/?note=file:beta.org");
    const links = page.locator(".read-on__link");
    await expect(links).toHaveCount(2);

    const earlier = (await links.nth(0).boundingBox())!;
    const later = (await links.nth(1).boundingBox())!;
    expect(later.y).toBeGreaterThanOrEqual(earlier.y + earlier.height);

    for (const box of [earlier, later]) {
      expect(box.width).toBeGreaterThan(0);
      expect(box.x + box.width).toBeLessThanOrEqual(375);
    }
    for (const index of [0, 1]) {
      expect(await isWhole(links.nth(index))).toBe(true);
      expect(await isHit(links.nth(index))).toBe(true);
    }
  });
});
