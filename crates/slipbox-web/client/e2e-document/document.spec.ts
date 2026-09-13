/*
 * The standalone document bundle in a real browser, served as static files with
 * no application and no API. Every request is intercepted: the page fetches the
 * bundle's own stylesheet and fonts from the served directory, and anything off
 * the origin is aborted and recorded, so a fixture cannot pass by reaching a CDN,
 * a font host or a daemon.
 */

import { expect, test } from "@playwright/test";

import {
  ORIGIN,
  clearHost,
  clipboardWrites,
  dispose,
  intents,
  isOwnOrigin,
  mount,
  openHost,
  pinHostScheme,
  pressRetainedOutside,
  recordClipboard,
  retain,
  retained,
  selectOwnText,
  selectionText,
  tones,
  update,
  type DocumentTones,
} from "./bundle.js";

const EQUATION = "\\[ \\int_0^1 x^2 \\, dx = \\frac{1}{3} \\]";

const SOURCE = [
  "A paragraph with *bold*, /italic/, =verbatim= and inline math \\(e^{i\\pi} = -1\\).",
  "",
  "See [[id:abc-123][the algorithm]] for the derivation.",
  "",
  "* A heading",
  "",
  "- a plain item",
  "- another, with \\(x^2\\) in it",
  "",
  "#+begin_src rust",
  "fn main() {}",
  "#+end_src",
  "",
  "#+begin_quote",
  "A quoted line.",
  "#+end_quote",
  "",
  EQUATION,
  "",
  "| quantity | value |",
  "|----------+-------|",
  "| \\(d\\)    | 0     |",
  "",
].join("\n");

const HOSTILE = [
  "A literal <script>window.__breached = true</script> and <b>bold</b>.",
  "",
  "Links: [[javascript:window.__breached = true][run it]],",
  "[[data:text/html,<script>window.__breached = true</script>][load it]],",
  "and [[file:notes/other.org][a file]].",
  "",
].join("\n");

function withEquation(tex: string): string {
  return SOURCE.replace(EQUATION, `\\[ ${tex} \\]`);
}

/** Wider than any reading measure, so the equation scrolls inside its box. */
const LONG_EQUATION = `${"x + ".repeat(60)}x`;

const AA_CONTRAST = 4.5;

/** WCAG relative luminance of a computed `rgb()` or `rgba()` tone. */
function luminance(tone: string): number {
  const weights = [0.2126, 0.7152, 0.0722];
  return (tone.match(/[\d.]+/g) ?? [])
    .slice(0, 3)
    .map(Number)
    .reduce((sum, channel, index) => {
      const unit = channel / 255;
      const linear = unit <= 0.03928 ? unit / 12.92 : ((unit + 0.055) / 1.055) ** 2.4;
      return sum + linear * weights[index]!;
    }, 0);
}

function contrast(ink: string, canvas: string): number {
  const one = luminance(ink);
  const two = luminance(canvas);
  return (Math.max(one, two) + 0.05) / (Math.min(one, two) + 0.05);
}

function readableTones(set: DocumentTones): void {
  expect(contrast(set.prose, set.canvas)).toBeGreaterThanOrEqual(AA_CONTRAST);
  expect(contrast(set.heading, set.canvas)).toBeGreaterThanOrEqual(AA_CONTRAST);
  expect(contrast(set.link, set.canvas)).toBeGreaterThanOrEqual(AA_CONTRAST);
  expect(contrast(set.codeInk, set.codeCanvas)).toBeGreaterThanOrEqual(AA_CONTRAST);
  expect(contrast(set.inlineCodeInk, set.inlineCodeCanvas)).toBeGreaterThanOrEqual(
    AA_CONTRAST,
  );
  // A cover on another tone than its canvas reads as a band across the equation.
  expect(set.mathCover).toBe(set.mathCanvas);
}

const LIGHT = {
  canvas: "rgb(255, 255, 255)",
  ink: "rgb(51, 51, 51)",
  code: "rgb(250, 250, 252)",
  link: "rgb(10, 110, 209)",
} as const;

const DARK = {
  canvas: "rgb(29, 29, 33)",
  ink: "rgb(215, 215, 218)",
  code: "rgb(22, 22, 26)",
  link: "rgb(90, 166, 234)",
} as const;

/** Requests aborted for leaving the origin; no journey may need one. */
let external: string[] = [];

test.beforeEach(async ({ page }) => {
  external = [];
  await page.route("**/*", (route) => {
    const url = route.request().url();
    if (isOwnOrigin(url)) {
      return route.continue();
    }
    external.push(url);
    return route.abort();
  });
});

test.afterEach(() => {
  expect(external).toEqual([]);
});

test("renders a document from the built files alone", async ({ page }) => {
  const requested: string[] = [];
  page.on("request", (request) => requested.push(request.url()));

  await openHost(page);

  await expect(page.getByRole("heading", { level: 2, name: "Blocks" })).toBeVisible();
  await expect(page.locator(".org-src[data-lang='rust'] .org-src__code")).toContainText(
    "println!",
  );
  await expect(page.locator(".org-quote p")).toHaveText("A quoted line.");
  await expect(page.locator(".org-table thead th").first()).toHaveText("Column");
  await expect(page.locator(".org-list li")).toHaveCount(2);
  await expect(page.locator(".org-math--display .katex")).toHaveCount(1);
  await expect(page.locator(".org-math--inline .katex")).toHaveCount(1);
  await expect(page.getByRole("button", { name: "Copy code to clipboard" })).toBeVisible();

  expect(requested.filter((url) => !isOwnOrigin(url))).toEqual([]);
  expect(requested.filter((url) => url.includes("/api/"))).toEqual([]);
  expect(requested.some((url) => url.endsWith("/document.js"))).toBe(true);
  expect(requested.some((url) => url.endsWith("/document.css"))).toBe(true);
});

test("typesets the document from its own stylesheet and bundled fonts", async ({
  page,
}) => {
  const requested: string[] = [];
  page.on("request", (request) => requested.push(request.url()));

  await openHost(page);

  const paragraph = page.locator(".org-paragraph").first();
  await expect(paragraph).toHaveCSS("font-size", "17px");
  // The body stack is the platform's own; nothing is downloaded for prose.
  expect(await paragraph.evaluate((el) => getComputedStyle(el).fontFamily)).toContain(
    "system-ui",
  );

  // The mono stack lands on the block; the agent's own rule for `pre` holds the
  // code element at the generic family, exactly as it does in the reader.
  await expect(page.locator(".org-src__code")).toHaveCSS("font-size", "13px");
  expect(
    await page.locator(".org-src").evaluate((el) => getComputedStyle(el).fontFamily),
  ).toContain("ui-monospace");

  // The scrolling edge shadows are org.css's, so their presence names its source.
  expect(
    await page
      .locator(".org-math--display")
      .evaluate((el) => getComputedStyle(el).backgroundImage),
  ).not.toBe("none");

  // KaTeX's fonts are files of the bundle, resolved relative to the stylesheet, so
  // typesetting loads them out of the served directory and not from a font host.
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  const fonts = await page.evaluate(() =>
    [...document.fonts].map((face) => face.family),
  );
  expect(fonts.some((family) => family.startsWith("KaTeX"))).toBe(true);
  const loaded = requested.filter((url) => /\/assets\/KaTeX_[^/]+\.woff2$/.test(url));
  expect(loaded.length).toBeGreaterThan(0);
  expect(
    loaded.filter(
      (url) => !isOwnOrigin(url) || !new URL(url).pathname.startsWith("/assets/KaTeX_"),
    ),
  ).toEqual([]);
});

test("paints each instance in the scheme its host asked for", async ({ page }) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, { handle: "light", source: SOURCE, theme: "light" });
  await mount(page, { handle: "dark", source: SOURCE, theme: "dark" });

  const surfaceOf = (handle: string): Promise<string> =>
    page
      .locator(`[data-handle='${handle}'] .org-src`)
      .evaluate((el) => getComputedStyle(el).backgroundColor);

  const light = await surfaceOf("light");
  const dark = await surfaceOf("dark");
  expect(light).not.toBe(dark);
  expect(light).toBe("rgb(250, 250, 252)");
  expect(dark).toBe("rgb(22, 22, 26)");

  await update(page, "light", { theme: "dark" });
  expect(await surfaceOf("light")).toBe(dark);

  await update(page, "light", { theme: "system" });
  expect(
    await page
      .locator("[data-handle='light'] .org-document-host")
      .getAttribute("data-theme"),
  ).toBeNull();
});

test("resolves prose, code and math tones on the container it mounts", async ({
  page,
}) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, { handle: "light", source: SOURCE, theme: "light", hrefPrefix: "#" });
  await mount(page, { handle: "dark", source: SOURCE, theme: "dark", hrefPrefix: "#" });

  const light = await tones(page, "light");
  const dark = await tones(page, "dark");

  expect(light.canvas).toBe(LIGHT.canvas);
  expect(light.prose).toBe(LIGHT.ink);
  expect(light.heading).toBe(LIGHT.ink);
  expect(light.codeInk).toBe(LIGHT.ink);
  expect(light.inlineCodeInk).toBe(LIGHT.ink);
  expect(light.codeCanvas).toBe(LIGHT.code);
  expect(light.inlineCodeCanvas).toBe(LIGHT.code);
  expect(light.link).toBe(LIGHT.link);

  expect(dark.canvas).toBe(DARK.canvas);
  expect(dark.prose).toBe(DARK.ink);
  expect(dark.heading).toBe(DARK.ink);
  expect(dark.codeInk).toBe(DARK.ink);
  expect(dark.inlineCodeInk).toBe(DARK.ink);
  expect(dark.codeCanvas).toBe(DARK.code);
  expect(dark.inlineCodeCanvas).toBe(DARK.code);
  expect(dark.link).toBe(DARK.link);

  readableTones(light);
  readableTones(dark);
});

test("keeps a document readable on a page pinned to the other scheme", async ({
  page,
}) => {
  await openHost(page);
  await clearHost(page);

  await pinHostScheme(page, "light");
  await mount(page, { handle: "dark", source: SOURCE, theme: "dark", hrefPrefix: "#" });
  const onLight = await tones(page, "dark");
  expect(onLight.canvas).toBe(DARK.canvas);
  expect(onLight.prose).toBe(DARK.ink);
  expect(onLight.codeCanvas).toBe(DARK.code);
  readableTones(onLight);

  // Neither the page nor a child the host keeps beside the mount is rethemed.
  const around = await page.evaluate(() => {
    const own = document.createElement("p");
    own.textContent = "the host's own";
    document.body.append(own);
    return {
      page: getComputedStyle(document.body).backgroundColor,
      sibling: getComputedStyle(own).color,
    };
  });
  expect(around).toEqual({ page: "rgb(250, 250, 252)", sibling: LIGHT.ink });

  await clearHost(page);
  await pinHostScheme(page, "dark");
  await mount(page, { handle: "light", source: SOURCE, theme: "light", hrefPrefix: "#" });
  const onDark = await tones(page, "light");
  expect(onDark.canvas).toBe(LIGHT.canvas);
  expect(onDark.prose).toBe(LIGHT.ink);
  expect(onDark.codeCanvas).toBe(LIGHT.code);
  readableTones(onDark);
  expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(
    "rgb(22, 22, 26)",
  );
});

test("holds opposite schemes side by side and repaints only what updated", async ({
  page,
}) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, { handle: "light", source: SOURCE, theme: "light", hrefPrefix: "#" });
  await mount(page, { handle: "dark", source: SOURCE, theme: "dark", hrefPrefix: "#" });

  const light = await tones(page, "light");
  const dark = await tones(page, "dark");
  expect(light.canvas).not.toBe(dark.canvas);
  expect(light.prose).not.toBe(dark.prose);
  readableTones(light);
  readableTones(dark);

  await update(page, "light", { theme: "dark" });
  expect(await tones(page, "light")).toEqual(dark);
  expect(await tones(page, "dark")).toEqual(dark);

  await update(page, "light", { theme: "light" });
  expect(await tones(page, "light")).toEqual(light);
  expect(await tones(page, "dark")).toEqual(dark);
});

test("follows the platform scheme on a page that pinned its own", async ({ page }) => {
  await openHost(page);
  await clearHost(page);
  await pinHostScheme(page, "light");
  await mount(page, { handle: "one", source: SOURCE, hrefPrefix: "#" });

  await page.emulateMedia({ colorScheme: "dark" });
  const dark = await tones(page, "one");
  expect(dark.canvas).toBe(DARK.canvas);
  expect(dark.prose).toBe(DARK.ink);
  expect(dark.codeCanvas).toBe(DARK.code);
  readableTones(dark);
  // The page keeps the scheme it pinned for itself.
  expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(
    "rgb(250, 250, 252)",
  );

  await page.emulateMedia({ colorScheme: "light" });
  const light = await tones(page, "one");
  expect(light.canvas).toBe(LIGHT.canvas);
  expect(light.prose).toBe(LIGHT.ink);
  readableTones(light);
});

test("covers a math edge shadow with the tone behind it, overflowing or not", async ({
  page,
}) => {
  await openHost(page);
  await clearHost(page);
  await pinHostScheme(page, "light");
  await mount(page, {
    handle: "wide",
    source: withEquation(LONG_EQUATION),
    theme: "dark",
    hrefPrefix: "#",
  });
  await mount(page, { handle: "narrow", source: SOURCE, theme: "dark", hrefPrefix: "#" });

  const wide = await tones(page, "wide");
  const narrow = await tones(page, "narrow");
  expect(wide.mathOverflows).toBe(true);
  expect(narrow.mathOverflows).toBe(false);
  for (const set of [wide, narrow]) {
    expect(set.mathCover).toBe(set.mathCanvas);
    expect(set.mathCover).toBe(DARK.canvas);
  }

  // The covers scroll with the equation while the shadows stay put, so a shadow
  // shows only where there is more to read.
  const scrolls = async (handle: string): Promise<number> =>
    page
      .locator(`[data-handle='${handle}'] .org-math--display`)
      .evaluate((el) => {
        el.scrollLeft = el.scrollWidth;
        return el.scrollLeft;
      });
  expect(
    await page
      .locator("[data-handle='wide'] .org-math--display")
      .evaluate((el) => getComputedStyle(el).backgroundAttachment),
  ).toBe("local, local, scroll, scroll");
  expect(await scrolls("wide")).toBeGreaterThan(0);
  expect(await scrolls("narrow")).toBe(0);
});

test("replaces content and the href grammar on update, in place", async ({ page }) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, {
    handle: "one",
    source: "First, linking to [[id:abc-123][a note]].\n",
    hrefPrefix: "/note/",
  });

  const container = page.locator(".org-document-host");
  await expect(container.locator("a.org-link")).toHaveAttribute("href", "/note/id:abc-123");

  await update(page, "one", {
    source: "Second, linking to [[id:def-456][another]].\n",
    hrefPrefix: "#",
  });

  await expect(container.locator(".org-paragraph")).toContainText("Second");
  await expect(container.locator("a.org-link")).toHaveAttribute("href", "#id:def-456");
  await expect(page.locator(".org-document-host")).toHaveCount(1);
});

test("keeps a preview's origin live across every update that spares it", async ({
  page,
}) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, {
    handle: "one",
    source: "See [[id:abc-123][the algorithm]].\n",
    hrefPrefix: "/note/",
  });

  const link = page.locator("a.org-link");
  await retain(page, "one", "a.org-link");
  await link.hover();
  await expect
    .poll(async () => (await intents(page)).map((intent) => intent.verb))
    .toEqual(["glance"]);
  let seen = (await intents(page)).length;

  await update(page, "one", { theme: "dark" });
  await update(page, "one", { hrefPrefix: "#" });
  await update(page, "one", { assets: { "file:media/plot.png": "media/plot.png" } });
  await update(page, "one", {});

  expect(await retained(page, "one", "a.org-link")).toEqual({
    connected: true,
    mounted: true,
    href: "#id:abc-123",
  });
  expect((await intents(page)).slice(seen)).toEqual([]);

  seen = (await intents(page)).length;
  await link.click();
  expect((await intents(page)).slice(seen).map((intent) => intent.verb)).toContain("pin");

  // Leave the link so hovering it again raises a new preview.
  await page.mouse.move(0, 0);
  await link.hover();
  await expect.poll(async () => (await intents(page)).at(-1)?.verb).toBe("glance");
  seen = (await intents(page)).length;
  await update(page, "one", { source: "Replaced.\n" });
  const after = (await intents(page)).slice(seen).map((intent) => intent.verb);
  expect(after[0]).toBe("dismiss");
  expect(new Set(after)).toEqual(new Set(["dismiss"]));
  expect((await retained(page, "one", "a.org-link")).connected).toBe(false);
});

test("refuses a press on a control the host kept past its content", async ({ page }) => {
  await openHost(page);
  await clearHost(page);
  await recordClipboard(page);
  await mount(page, {
    handle: "one",
    source: "#+begin_src rust\nPRIVATE_OLD_SOURCE\n#+end_src\n",
  });
  await mount(page, { handle: "two", source: "#+begin_src rust\nfn other() {}\n#+end_src\n" });
  await selectOwnText(page, "what the host selected");

  await retain(page, "one", ".org-src__copy");
  await dispose(page, "one");
  await pressRetainedOutside(page);

  expect(await clipboardWrites(page)).toEqual([]);
  expect(await selectionText(page)).toBe("what the host selected");

  await retain(page, "two", ".org-src__copy");
  await update(page, "two", { source: "#+begin_src rust\nfn third() {}\n#+end_src\n" });
  await pressRetainedOutside(page);
  expect(await clipboardWrites(page)).toEqual([]);

  await page.locator("[data-handle='two'] .org-src__copy").click();
  await expect
    .poll(async () => (await clipboardWrites(page)).map((text) => text.trim()))
    .toEqual(["fn third() {}"]);
  await expect(page.locator("[data-handle='two'] .org-src__copy")).toHaveText("Copied");
});

test("keeps instances independent and disposes only its own nodes", async ({ page }) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, { handle: "one", source: "One.\n" });
  await mount(page, { handle: "two", source: "Two.\n" });

  await page.evaluate(() => {
    const kept = document.createElement("p");
    kept.id = "host-own";
    document.querySelector("[data-handle='one']")?.append(kept);
  });

  await update(page, "one", { source: "One, revised.\n" });
  await expect(page.locator("[data-handle='two'] .org-document")).toHaveText("Two.");

  await dispose(page, "one");
  await expect(page.locator("[data-handle='one'] .org-document")).toHaveCount(0);
  await expect(page.locator("#host-own")).toHaveCount(1);
  await expect(page.locator("[data-handle='two'] .org-document")).toHaveText("Two.");
});

test("reports gestures as caller-owned intents and never navigates itself", async ({
  page,
}) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, {
    handle: "one",
    source: "See [[id:abc-123][the algorithm]].\n",
    hrefPrefix: "/note/",
  });

  const link = page.locator("a.org-link");
  await link.hover();
  await expect
    .poll(async () => (await intents(page)).map((intent) => intent.verb))
    .toEqual(["glance"]);
  const glance = (await intents(page))[0]!;
  expect(glance.gesture).toBe("hover");
  expect(glance.reference).toBe("id:abc-123");
  expect(glance.originIsLink).toBe(true);

  // A press focuses the anchor first, so the browser's own focus raises a second
  // glance; committing dismisses what is raised, pins, then drops focus, which
  // dismisses again.
  let seen = (await intents(page)).length;
  await link.click();
  const afterClick = (await intents(page)).slice(seen);
  expect(afterClick.map((intent) => intent.verb)).toEqual([
    "glance",
    "dismiss",
    "pin",
    "dismiss",
  ]);
  expect(afterClick[0]?.gesture).toBe("focus");

  expect(new URL(page.url()).pathname).toBe("/host.html");

  seen = (await intents(page)).length;
  await link.click({ modifiers: ["Alt"] });
  expect((await intents(page)).slice(seen).map((intent) => intent.verb)).toEqual([
    "glance",
    "dismiss",
    "go",
    "dismiss",
  ]);

  seen = (await intents(page)).length;
  await link.focus();
  await link.press("Escape");
  expect((await intents(page)).slice(seen).map((intent) => intent.verb)).toEqual([
    "glance",
    "dismiss",
  ]);
  expect(new URL(page.url()).pathname).toBe("/host.html");
});

test("escapes hostile content and leaves an unfollowable target inert", async ({
  page,
}) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, { handle: "one", source: HOSTILE });

  const rendered = page.locator(".org-document");
  await expect(rendered).toContainText("<script>window.__breached = true</script>");
  await expect(rendered).toContainText("<b>bold</b>");
  await expect(rendered.locator("script")).toHaveCount(0);
  await expect(rendered.locator("b")).toHaveCount(0);
  await expect(rendered.locator("a")).toHaveCount(0);
  await expect(rendered.locator(".org-link--inert")).toHaveCount(3);
  expect(await page.evaluate(() => "__breached" in window)).toBe(false);
});

test("anchors an asset the host resolved and refuses a hostile resolution", async ({
  page,
}) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, {
    handle: "one",
    source: "See [[file:media/plot.png][the plot]].\n",
    assets: { "file:media/plot.png": "media/plot.png" },
  });

  const asset = page.locator("a.org-link--asset");
  await expect(asset).toHaveAttribute("href", "media/plot.png");
  // Relative, so the resolved URL stays inside whatever origin serves the bundle.
  expect(await asset.evaluate((el) => (el as HTMLAnchorElement).href)).toBe(
    `${ORIGIN}/media/plot.png`,
  );

  await update(page, "one", { assets: { "file:media/plot.png": "javascript:alert(1)" } });
  await expect(page.locator(".org-document a")).toHaveCount(0);
  await expect(page.locator(".org-link--inert")).toHaveCount(1);

  await update(page, "one", { assets: { "file:media/plot.png": "//example.org/plot.png" } });
  await expect(page.locator(".org-document a")).toHaveCount(0);
});

test("refuses a resolution a browser would read as another authority", async ({
  page,
}) => {
  await openHost(page);
  await clearHost(page);
  await mount(page, {
    handle: "one",
    source: "See [[file:media/plot.png][the plot]].\n",
    assets: { "file:media/plot.png": "media/plot.png" },
  });
  await expect(page.locator("a.org-link--asset")).toHaveAttribute("href", "media/plot.png");

  for (const hostile of [
    "\\\\example.org/plot.png",
    "/\\example.org/plot.png",
    "\\/example.org/plot.png",
    "\t\\\\example.org/plot.png",
    "HTTPS:\\\\example.org/plot.png",
  ]) {
    await update(page, "one", { assets: { "file:media/plot.png": hostile } });
    await expect(page.locator(".org-document a")).toHaveCount(0);
    await expect(page.locator(".org-link--inert")).toHaveCount(1);
  }

  await update(page, "one", { assets: { "file:media/plot.png": "media/plot.png" } });
  expect(
    await page.locator("a.org-link--asset").evaluate((el) => (el as HTMLAnchorElement).href),
  ).toBe(`${ORIGIN}/media/plot.png`);
});

test("names this origin by parsed equality rather than by prefix", async ({ page }) => {
  await openHost(page);

  const resolved = await page.evaluate((candidates) => {
    const probe = document.createElement("a");
    return candidates.map((candidate) => {
      probe.href = candidate;
      return probe.href;
    });
  }, ["\\\\example.org/plot.png", "/\\example.org/plot.png", "\\/example.org/plot.png"]);

  // Each refused form is an authority off this origin once a browser parses it.
  for (const href of resolved) {
    expect(isOwnOrigin(href)).toBe(false);
    expect(new URL(href).origin).toBe("http://example.org");
  }

  // A prefix test admits both of these; parsed equality admits neither.
  for (const near of [`${ORIGIN}0/plot.png`, `${ORIGIN}@evil.example/plot.png`]) {
    expect(near.startsWith(ORIGIN)).toBe(true);
    expect(isOwnOrigin(near)).toBe(false);
  }
  expect(isOwnOrigin(`${ORIGIN}/assets/KaTeX_Main-Regular.woff2`)).toBe(true);
  expect(isOwnOrigin("not a url")).toBe(false);
});

test("shows TeX that defeats KaTeX as its own source", async ({ page }) => {
  await openHost(page);
  await clearHost(page);
  const runaway = "\\sqrt{".repeat(2000);
  await mount(page, {
    handle: "one",
    source: `Broken \\(\\frac{1\\) and \\(${runaway}\\) here.\n`,
  });

  await expect(page.locator(".org-math .katex-error")).toHaveCount(2);
  await expect(page.locator(".org-math .katex-error").first()).toHaveText("\\frac{1");
  await expect(page.locator(".org-document")).toContainText("here.");
});

for (const width of [420, 1400]) {
  for (const theme of ["light", "dark"] as const) {
    test(`holds the reading measure at ${width}px in ${theme}`, async ({
      page,
    }, testInfo) => {
      await page.setViewportSize({ width, height: 900 });
      await openHost(page);
      await clearHost(page);
      await mount(page, { handle: "one", source: SOURCE, theme });

      // The host column caps at the token measure and pads inside it, so the prose
      // fills what is left of it.
      const column = (await page.locator("[data-handle='one']").boundingBox())!;
      const paragraph = (await page.locator(".org-paragraph").first().boundingBox())!;
      expect(column.width).toBeLessThanOrEqual(625);
      expect(Math.round(paragraph.width)).toBe(Math.round(column.width) - 64);
      if (width > 625) {
        expect(Math.round(column.width)).toBe(625);
      }

      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
      );
      expect(overflow).toBe(0);

      await page.screenshot({
        path: testInfo.outputPath(`${theme}-${width}.png`),
        fullPage: true,
      });
    });
  }
}
