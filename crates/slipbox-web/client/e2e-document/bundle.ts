/*
 * Driving the built document bundle from a page that holds nothing else. Its
 * hooks are functions, which cannot cross into the page, so a spec describes them
 * with plain data and this module builds them inside the page.
 */

import type { Page } from "@playwright/test";

/** The origin the preview server owns; every request must stay inside it. */
export const ORIGIN = "http://localhost:4174";

export function isOwnOrigin(url: string): boolean {
  try {
    return new URL(url).origin === ORIGIN;
  } catch {
    return false;
  }
}

const ENTRY = "./document.js";

export interface MountSpec {
  readonly handle: string;
  readonly source: string;
  readonly theme?: "system" | "light" | "dark";
  readonly baseLevel?: number;
  /** Present to register an href hook returning this prefix plus the reference. */
  readonly hrefPrefix?: string;
  /** Present to register a resolver answering from this table. */
  readonly assets?: Record<string, string>;
}

/** One recorded intent, flattened to what survives the page boundary. */
export interface RecordedIntent {
  readonly verb: string;
  readonly reference: string | null;
  readonly gesture: string | null;
  readonly originIsLink: boolean;
  readonly keys: readonly string[];
}

export async function openHost(page: Page): Promise<void> {
  await page.goto("/host.html");
  await page.waitForSelector(".org-document");
}

export async function clearHost(page: Page): Promise<void> {
  await page.evaluate(() => {
    document.body.replaceChildren();
    delete window.__slipboxDocuments;
    delete window.__slipboxIntents;
  });
}

export async function mount(page: Page, spec: MountSpec): Promise<void> {
  await page.evaluate(
    async ([entry, spec]) => {
      const bundle = await import(entry);
      const handles = (window.__slipboxDocuments ??= {});
      const intents = (window.__slipboxIntents ??= []);

      const host = document.createElement("div");
      host.className = "host-column";
      host.dataset.handle = spec.handle;
      document.body.append(host);

      handles[spec.handle] = bundle.mountOrgDocument(host, {
        content: { source: spec.source, baseLevel: spec.baseLevel },
        theme: spec.theme,
        onIntent: (intent: Record<string, unknown>) =>
          intents.push({
            verb: String(intent["verb"]),
            reference:
              (intent["link"] as { reference?: string } | undefined)?.reference ?? null,
            gesture: (intent["gesture"] as string | undefined) ?? null,
            originIsLink: (intent["origin"] as Element | undefined)?.tagName === "A",
            keys: Object.keys(intent),
          }),
        ...(spec.hrefPrefix === undefined
          ? {}
          : {
              href: (link: { reference: string }) => `${spec.hrefPrefix}${link.reference}`,
            }),
        ...(spec.assets === undefined
          ? {}
          : {
              resolveAsset: (target: string) => spec.assets?.[target] ?? null,
            }),
      });
    },
    [ENTRY, spec] as const,
  );
}

export async function update(
  page: Page,
  handle: string,
  patch: Partial<Omit<MountSpec, "handle">>,
): Promise<void> {
  await page.evaluate(
    ([handle, patch]) => {
      window.__slipboxDocuments?.[handle]?.update({
        ...(patch.source === undefined
          ? {}
          : { content: { source: patch.source, baseLevel: patch.baseLevel } }),
        ...(patch.theme === undefined ? {} : { theme: patch.theme }),
        ...(patch.hrefPrefix === undefined
          ? {}
          : {
              href: (link: { reference: string }) => `${patch.hrefPrefix}${link.reference}`,
            }),
        ...(patch.assets === undefined
          ? {}
          : { resolveAsset: (target: string) => patch.assets?.[target] ?? null }),
      });
    },
    [handle, patch] as const,
  );
}

export async function dispose(page: Page, handle: string): Promise<void> {
  await page.evaluate((handle) => {
    window.__slipboxDocuments?.[handle]?.dispose();
  }, handle);
}

export async function intents(page: Page): Promise<RecordedIntent[]> {
  return page.evaluate(() => window.__slipboxIntents ?? []);
}

export async function retain(
  page: Page,
  handle: string,
  selector: string,
): Promise<void> {
  await page.evaluate(
    ([handle, selector]) => {
      const found = document.querySelector(`[data-handle='${handle}'] ${selector}`);
      if (!(found instanceof HTMLElement)) {
        throw new Error(`${handle} rendered no ${selector}`);
      }
      window.__slipboxRetained = found;
    },
    [handle, selector] as const,
  );
}

export interface RetainedState {
  readonly connected: boolean;
  /** Whether the mount still renders the retained element rather than a new one. */
  readonly mounted: boolean;
  readonly href: string | null;
}

export async function retained(
  page: Page,
  handle: string,
  selector: string,
): Promise<RetainedState> {
  return page.evaluate(
    ([handle, selector]) => {
      const kept = window.__slipboxRetained;
      if (!(kept instanceof HTMLElement)) {
        throw new Error("nothing retained");
      }
      return {
        connected: kept.isConnected,
        mounted:
          document.querySelector(`[data-handle='${handle}'] ${selector}`) === kept,
        href: kept.getAttribute("href"),
      };
    },
    [handle, selector] as const,
  );
}

export async function pressRetainedOutside(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const kept = window.__slipboxRetained;
    if (!(kept instanceof HTMLElement)) {
      throw new Error("nothing retained");
    }
    document.body.append(kept);
    kept.click();
    await Promise.resolve();
    kept.remove();
  });
}

/** Record scripted clipboard writes rather than performing them. */
export async function recordClipboard(page: Page): Promise<void> {
  await page.evaluate(() => {
    const written: string[] = (window.__slipboxCopies = []);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) => {
          written.push(text);
          return Promise.resolve();
        },
      },
    });
  });
}

export async function clipboardWrites(page: Page): Promise<string[]> {
  return page.evaluate(() => window.__slipboxCopies ?? []);
}

export async function selectOwnText(page: Page, text: string): Promise<void> {
  await page.evaluate((text) => {
    const own = document.createElement("p");
    own.textContent = text;
    document.body.append(own);
    const range = document.createRange();
    range.selectNodeContents(own);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  }, text);
}

export async function selectionText(page: Page): Promise<string> {
  return page.evaluate(() => window.getSelection()?.toString() ?? "");
}

/** Pin a scheme on the page around the mounts, as a host with its own theme does. */
export async function pinHostScheme(
  page: Page,
  theme: "light" | "dark" | null,
): Promise<void> {
  await page.evaluate((theme) => {
    if (theme === null) {
      delete document.documentElement.dataset.theme;
    } else {
      document.documentElement.dataset.theme = theme;
    }
  }, theme);
}

/** What a mounted document resolved for the tones a reader actually sees. */
export interface DocumentTones {
  /** The container's own background, or the tone painted behind it if it has none. */
  readonly canvas: string;
  readonly prose: string;
  readonly heading: string;
  readonly link: string;
  readonly inlineCodeCanvas: string;
  readonly inlineCodeInk: string;
  readonly codeCanvas: string;
  readonly codeInk: string;
  /**
   * The opaque tone the display-math cover gradients start from, which must match
   * the tone behind the equation or the covers read as bands.
   */
  readonly mathCover: string;
  /** The tone actually painted behind the display math. */
  readonly mathCanvas: string;
  readonly mathOverflows: boolean;
}

export async function tones(page: Page, handle: string): Promise<DocumentTones> {
  return page.evaluate((handle) => {
    const host = document.querySelector(`[data-handle='${handle}'] .org-document-host`);
    if (!(host instanceof HTMLElement)) {
      throw new Error(`no mounted document for handle ${handle}`);
    }
    const find = (selector: string): HTMLElement => {
      const found = host.querySelector(selector);
      if (!(found instanceof HTMLElement)) {
        throw new Error(`${handle} rendered no ${selector}`);
      }
      return found;
    };
    const painted = (from: Element): string => {
      for (let node: Element | null = from; node !== null; node = node.parentElement) {
        const tone = getComputedStyle(node).backgroundColor;
        if (tone !== "rgba(0, 0, 0, 0)" && tone !== "transparent") {
          return tone;
        }
      }
      return "rgb(255, 255, 255)";
    };
    const math = find(".org-math--display");
    return {
      canvas: painted(host),
      prose: getComputedStyle(find(".org-paragraph")).color,
      heading: getComputedStyle(find(".org-heading")).color,
      link: getComputedStyle(find("a.org-link")).color,
      inlineCodeCanvas: getComputedStyle(find(".org-verbatim")).backgroundColor,
      inlineCodeInk: getComputedStyle(find(".org-verbatim")).color,
      codeCanvas: getComputedStyle(find(".org-src")).backgroundColor,
      codeInk: getComputedStyle(find(".org-src__code")).color,
      mathCover:
        getComputedStyle(math).backgroundImage.match(/rgba?\([^)]*\)/)?.[0] ?? "",
      mathCanvas: painted(math.parentElement ?? math),
      mathOverflows: math.scrollWidth > math.clientWidth,
    };
  }, handle);
}

declare global {
  interface Window {
    __slipboxDocuments?: Record<
      string,
      { update: (patch: unknown) => void; dispose: () => void }
    >;
    __slipboxIntents?: RecordedIntent[];
    __slipboxRetained?: HTMLElement;
    __slipboxCopies?: string[];
  }
}
