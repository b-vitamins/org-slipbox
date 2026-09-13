import { afterEach, describe, expect, it, vi } from "vitest";

import { mountOrgDocument } from "./mount.jsx";
import type { OrgDocumentIntent } from "./contract.js";

/** Answer the link grammar's hover query as a cursor-driven browser does. */
function stubHoverPointer(): void {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: false,
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

function hostElement(): HTMLElement {
  const host = document.createElement("div");
  document.body.append(host);
  return host;
}

function documentOf(host: HTMLElement): HTMLElement {
  return host.querySelector(".org-document") as HTMLElement;
}

function copyControlOf(host: HTMLElement): HTMLButtonElement {
  return documentOf(host).querySelector(".org-src__copy") as HTMLButtonElement;
}

function pendingClipboard(): { settle: () => void; refuse: () => void } {
  let resolve: () => void = () => {};
  let reject: (reason: Error) => void = () => {};
  vi.stubGlobal("navigator", {
    clipboard: {
      writeText: () =>
        new Promise<void>((ok, no) => {
          resolve = ok;
          reject = no;
        }),
    },
  });
  return { settle: () => resolve(), refuse: () => reject(new Error("denied")) };
}

function recordingClipboard(): string[] {
  const written: string[] = [];
  vi.stubGlobal("navigator", {
    clipboard: {
      writeText: (text: string) => {
        written.push(text);
        return Promise.resolve();
      },
    },
  });
  return written;
}

function selectOwnText(text: string): HTMLElement {
  const own = document.createElement("p");
  own.textContent = text;
  document.body.append(own);
  const range = document.createRange();
  range.selectNodeContents(own);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
  return own;
}

const CODE = "#+begin_src rust\nfn main() {}\n#+end_src\n";
const OTHER_CODE = "#+begin_src rust\nfn other() {}\n#+end_src\n";
const LINKED = "see [[id:abc-123][the algorithm]]\n";
const HEADED_LINK = "* see [[id:abc-123][the algorithm]]\n";

describe("mountOrgDocument", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    document.body.replaceChildren();
  });

  it("renders the source into a container of its own, beside what the host had", () => {
    const host = hostElement();
    const kept = document.createElement("p");
    kept.textContent = "the host's own";
    host.append(kept);

    const handle = mountOrgDocument(host, {
      content: { source: "* Section\n\nA /clear/ line.\n" },
    });

    const container = host.querySelector(".org-document-host") as HTMLElement;
    expect(container).not.toBeNull();
    expect(container.querySelector("h2.org-heading")?.textContent).toBe("Section");
    expect(container.querySelector("em")?.textContent).toBe("clear");
    expect(host.contains(kept)).toBe(true);

    handle.dispose();
    expect(host.querySelector(".org-document-host")).toBeNull();
    expect(host.contains(kept)).toBe(true);
  });

  it("heads the document where the host asked it to", () => {
    const host = hostElement();
    mountOrgDocument(host, {
      content: { source: "* Section\n", baseLevel: 4 },
    });

    expect(documentOf(host).querySelector("h4.org-heading")).not.toBeNull();
  });

  it("replaces the content on update without remounting the container", () => {
    const host = hostElement();
    const handle = mountOrgDocument(host, { content: { source: "First.\n" } });
    const container = host.querySelector(".org-document-host");

    handle.update({ content: { source: "Second, with *weight*.\n" } });

    expect(host.querySelector(".org-document-host")).toBe(container);
    expect(documentOf(host).textContent).toBe("Second, with weight.");
    expect(documentOf(host).querySelector("strong")?.textContent).toBe("weight");
  });

  it("selects a scheme on its own container and follows the platform by default", () => {
    const host = hostElement();
    const handle = mountOrgDocument(host, { content: { source: "A line.\n" } });
    const container = host.querySelector(".org-document-host") as HTMLElement;

    expect(container.hasAttribute("data-theme")).toBe(false);

    handle.update({ theme: "dark" });
    expect(container.getAttribute("data-theme")).toBe("dark");

    handle.update({ theme: "light" });
    expect(container.getAttribute("data-theme")).toBe("light");

    handle.update({ theme: "system" });
    expect(container.hasAttribute("data-theme")).toBe(false);
  });

  it("keeps the options an update does not name", () => {
    const host = hostElement();
    const intents: OrgDocumentIntent[] = [];
    const handle = mountOrgDocument(host, {
      content: { source: "see [[id:abc-123][the algorithm]]\n" },
      theme: "dark",
      href: (link) => `/note/${link.reference}`,
      onIntent: (intent) => intents.push(intent),
    });
    const container = host.querySelector(".org-document-host") as HTMLElement;

    handle.update({ content: { source: "see [[id:def-456][the other]]\n" } });

    expect(container.getAttribute("data-theme")).toBe("dark");
    const link = documentOf(host).querySelector("a.org-link") as HTMLAnchorElement;
    expect(link.getAttribute("href")).toBe("/note/id:def-456");

    link.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, detail: 1 }));
    expect(intents.at(-1)).toEqual({
      verb: "pin",
      link: { id: "def-456", target: "id:def-456", reference: "id:def-456" },
    });
  });

  it("keeps instances on one page independent", () => {
    const first = hostElement();
    const second = hostElement();
    const one = mountOrgDocument(first, { content: { source: "One.\n" } });
    mountOrgDocument(second, { content: { source: "Two.\n" }, theme: "dark" });

    one.update({ content: { source: "One, revised.\n" }, theme: "light" });

    expect(documentOf(first).textContent).toBe("One, revised.");
    expect(documentOf(second).textContent).toBe("Two.");
    expect(
      (first.querySelector(".org-document-host") as HTMLElement).getAttribute("data-theme"),
    ).toBe("light");
    expect(
      (second.querySelector(".org-document-host") as HTMLElement).getAttribute("data-theme"),
    ).toBe("dark");

    one.dispose();
    expect(first.querySelector(".org-document")).toBeNull();
    expect(documentOf(second).textContent).toBe("Two.");
  });

  it("releases a timer a rendered control left pending", () => {
    vi.useFakeTimers();
    const scheduled = vi.spyOn(window, "setTimeout");
    const cleared = vi.spyOn(window, "clearTimeout");
    const host = hostElement();
    const handle = mountOrgDocument(host, {
      content: { source: "#+begin_src rust\nfn main() {}\n#+end_src\n" },
    });
    expect(scheduled).not.toHaveBeenCalled();

    // jsdom exposes no clipboard, so the refusal path selects the code and
    // schedules its revert synchronously. Selecting queues a task of jsdom's
    // own, so the revert is the last timer the press schedules, not the only one.
    const copy = documentOf(host).querySelector(".org-src__copy") as HTMLButtonElement;
    copy.click();
    const revert: unknown = scheduled.mock.results.at(-1)?.value;
    expect(revert).toBeDefined();

    handle.dispose();
    expect(cleared).toHaveBeenCalledWith(revert);
  });

  it("drops a copy that succeeds after disposal, leaving another document alone", async () => {
    vi.useFakeTimers();
    const clipboard = pendingClipboard();
    const scheduled = vi.spyOn(window, "setTimeout");
    const first = hostElement();
    const second = hostElement();
    const one = mountOrgDocument(first, { content: { source: CODE } });
    mountOrgDocument(second, { content: { source: CODE } });

    copyControlOf(first).click();
    one.dispose();
    const timers = scheduled.mock.calls.length;
    clipboard.settle();
    await vi.advanceTimersByTimeAsync(0);

    expect(scheduled.mock.calls.length).toBe(timers);
    expect(copyControlOf(second).textContent).toBe("Copy");
  });

  it("drops a refusal that arrives after the content was replaced", async () => {
    vi.useFakeTimers();
    const clipboard = pendingClipboard();
    const scheduled = vi.spyOn(window, "setTimeout");
    const host = hostElement();
    const handle = mountOrgDocument(host, { content: { source: CODE } });

    copyControlOf(host).click();
    const own = selectOwnText("what the host selected");
    handle.update({ content: { source: "Replaced.\n" } });
    const timers = scheduled.mock.calls.length;
    clipboard.refuse();
    await vi.advanceTimersByTimeAsync(0);

    expect(scheduled.mock.calls.length).toBe(timers);
    expect(window.getSelection()?.toString()).toBe("what the host selected");
    expect(documentOf(host).textContent).toBe("Replaced.");
    own.remove();
  });

  it("retires an outstanding glance to the hook that raised it", () => {
    stubHoverPointer();
    const host = hostElement();
    const raised: OrgDocumentIntent[] = [];
    const later: OrgDocumentIntent[] = [];
    const handle = mountOrgDocument(host, {
      content: { source: LINKED },
      onIntent: (intent) => raised.push(intent),
    });

    documentOf(host)
      .querySelector("a.org-link")
      ?.dispatchEvent(new MouseEvent("mouseenter"));
    expect(raised.map((intent) => intent.verb)).toEqual(["glance"]);

    handle.update({
      content: { source: "Nothing to glance at.\n" },
      onIntent: (intent) => later.push(intent),
    });

    expect(raised.map((intent) => intent.verb)).toEqual(["glance", "dismiss"]);
    expect(later).toEqual([]);
  });

  it("holds a raised glance through a theme, href, asset or empty update", () => {
    stubHoverPointer();
    const host = hostElement();
    const intents: OrgDocumentIntent[] = [];
    const handle = mountOrgDocument(host, {
      content: { source: LINKED },
      href: (link) => `/note/${link.reference}`,
      onIntent: (intent) => intents.push(intent),
    });
    const origin = documentOf(host).querySelector("a.org-link") as HTMLAnchorElement;

    origin.dispatchEvent(new MouseEvent("mouseenter"));
    expect(intents).toMatchObject([{ verb: "glance", origin }]);

    handle.update({ theme: "dark" });
    handle.update({ href: (link) => `#${link.reference}` });
    handle.update({ resolveAsset: () => null });
    handle.update({});

    expect(intents.map((intent) => intent.verb)).toEqual(["glance"]);
    expect(origin.isConnected).toBe(true);
    expect(documentOf(host).querySelector("a.org-link")).toBe(origin);
    expect(origin.getAttribute("href")).toBe("#id:abc-123");
    origin.dispatchEvent(
      new MouseEvent("click", { bubbles: true, cancelable: true, detail: 1 }),
    );
    expect(intents.map((intent) => intent.verb)).toEqual(["glance", "dismiss", "pin"]);
  });

  it("withdraws a glance to the hook an update is replacing", () => {
    stubHoverPointer();
    const host = hostElement();
    const raised: OrgDocumentIntent[] = [];
    const later: OrgDocumentIntent[] = [];
    const handle = mountOrgDocument(host, {
      content: { source: LINKED },
      onIntent: (intent) => raised.push(intent),
    });
    const origin = documentOf(host).querySelector("a.org-link") as HTMLAnchorElement;

    origin.dispatchEvent(new MouseEvent("mouseenter"));
    handle.update({ onIntent: (intent) => later.push(intent) });

    expect(raised.map((intent) => intent.verb)).toEqual(["glance", "dismiss"]);
    expect(later).toEqual([]);
    expect(documentOf(host).querySelector("a.org-link")).toBe(origin);
  });

  it("withdraws a glance before a new base level rebuilds its origin", () => {
    stubHoverPointer();
    const host = hostElement();
    const intents: OrgDocumentIntent[] = [];
    const handle = mountOrgDocument(host, {
      content: { source: HEADED_LINK },
      onIntent: (intent) => intents.push(intent),
    });
    const origin = documentOf(host).querySelector("a.org-link") as HTMLAnchorElement;

    origin.dispatchEvent(new MouseEvent("mouseenter"));
    handle.update({ content: { source: HEADED_LINK, baseLevel: 4 } });

    expect(intents.map((intent) => intent.verb)).toEqual(["glance", "dismiss"]);
    expect(documentOf(host).querySelector("h4.org-heading")).not.toBeNull();
    expect(documentOf(host).querySelector("a.org-link")).not.toBe(origin);
    expect(origin.isConnected).toBe(false);
  });

  it("refuses a press on a control kept past disposal, before the clipboard", async () => {
    vi.useFakeTimers();
    const written = recordingClipboard();
    const scheduled = vi.spyOn(window, "setTimeout");
    const first = hostElement();
    const second = hostElement();
    const one = mountOrgDocument(first, { content: { source: CODE } });
    mountOrgDocument(second, { content: { source: OTHER_CODE } });
    const kept = copyControlOf(first);
    const own = selectOwnText("what the host selected");

    one.dispose();
    document.body.append(kept);
    const timers = scheduled.mock.calls.length;
    kept.click();
    await vi.advanceTimersByTimeAsync(0);

    expect(written).toEqual([]);
    expect(scheduled.mock.calls.length).toBe(timers);
    expect(kept.textContent).toBe("Copy");
    expect(window.getSelection()?.toString()).toBe("what the host selected");
    expect(copyControlOf(second).textContent).toBe("Copy");

    copyControlOf(second).click();
    await vi.advanceTimersByTimeAsync(0);
    expect(written).toHaveLength(1);
    expect(written[0]).toContain("fn other()");

    kept.remove();
    own.remove();
  });

  it("refuses a press on a control whose content has been replaced", async () => {
    vi.useFakeTimers();
    const written = recordingClipboard();
    const host = hostElement();
    const handle = mountOrgDocument(host, { content: { source: CODE } });
    const kept = copyControlOf(host);

    handle.update({ content: { source: OTHER_CODE } });
    document.body.append(kept);
    kept.click();
    await vi.advanceTimersByTimeAsync(0);

    expect(written).toEqual([]);

    copyControlOf(host).click();
    await vi.advanceTimersByTimeAsync(0);
    expect(written).toHaveLength(1);
    expect(written[0]).toContain("fn other()");
    expect(copyControlOf(host).textContent).toBe("Copied");

    kept.remove();
  });

  it("retires an outstanding glance on disposal", () => {
    stubHoverPointer();
    const host = hostElement();
    const intents: OrgDocumentIntent[] = [];
    const handle = mountOrgDocument(host, {
      content: { source: LINKED },
      onIntent: (intent) => intents.push(intent),
    });

    documentOf(host)
      .querySelector("a.org-link")
      ?.dispatchEvent(new MouseEvent("mouseenter"));
    handle.dispose();

    expect(intents.map((intent) => intent.verb)).toEqual(["glance", "dismiss"]);
  });

  it("reports nothing for a gesture on a link the document no longer holds", () => {
    stubHoverPointer();
    const host = hostElement();
    const intents: OrgDocumentIntent[] = [];
    const handle = mountOrgDocument(host, {
      content: { source: LINKED },
      onIntent: (intent) => intents.push(intent),
    });
    const stale = documentOf(host).querySelector("a.org-link") as HTMLAnchorElement;

    handle.update({ content: { source: "see [[id:def-456][the other]]\n" } });
    // A host may still hold the element it drew a card against, and a delegated
    // handler reaches it again once it is back in the page.
    document.body.append(stale);
    const seen = intents.length;
    stale.dispatchEvent(new MouseEvent("mouseenter"));
    stale.dispatchEvent(
      new MouseEvent("click", { bubbles: true, cancelable: true, detail: 1 }),
    );

    expect(intents.slice(seen)).toEqual([]);
    stale.remove();
  });

  it("keeps a disposed document's stale link out of a live instance", () => {
    stubHoverPointer();
    const first = hostElement();
    const second = hostElement();
    const other: OrgDocumentIntent[] = [];
    const one = mountOrgDocument(first, { content: { source: LINKED } });
    mountOrgDocument(second, {
      content: { source: LINKED },
      onIntent: (intent) => other.push(intent),
    });
    const stale = documentOf(first).querySelector("a.org-link") as HTMLAnchorElement;

    one.dispose();
    document.body.append(stale);
    stale.dispatchEvent(new MouseEvent("mouseenter"));
    stale.dispatchEvent(
      new MouseEvent("click", { bubbles: true, cancelable: true, detail: 1 }),
    );

    expect(other).toEqual([]);
    expect(documentOf(second).querySelector("a.org-link")).not.toBeNull();
    stale.remove();
  });

  it("takes a second dispose and a later update as inert", () => {
    const host = hostElement();
    const handle = mountOrgDocument(host, { content: { source: "A line.\n" } });

    handle.dispose();
    handle.dispose();
    handle.update({ content: { source: "Ignored.\n" } });

    expect(host.querySelector(".org-document-host")).toBeNull();
    expect(host.textContent).toBe("");
  });

  it("reports every gesture as data the host reads, holding no callback", () => {
    stubHoverPointer();
    const host = hostElement();
    const intents: OrgDocumentIntent[] = [];
    mountOrgDocument(host, {
      content: { source: "see [[id:abc-123][the algorithm]]\n" },
      onIntent: (intent) => intents.push(intent),
    });
    const link = documentOf(host).querySelector("a.org-link") as HTMLAnchorElement;

    link.dispatchEvent(new MouseEvent("mouseenter"));
    const glance = intents[0]!;
    expect(glance.verb).toBe("glance");
    expect(Object.values(glance).some((value) => typeof value === "function")).toBe(false);
    expect(glance).toMatchObject({
      gesture: "hover",
      origin: link,
      link: { id: "abc-123", target: "id:abc-123", reference: "id:abc-123" },
    });

    link.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, detail: 1 }));
    expect(intents.map((intent) => intent.verb)).toEqual([
      "glance",
      "dismiss",
      "pin",
    ]);

    link.dispatchEvent(
      new MouseEvent("click", { bubbles: true, cancelable: true, detail: 1, altKey: true }),
    );
    expect(intents.at(-1)?.verb).toBe("go");
  });

  it("drops gestures where the host registered no hook", () => {
    stubHoverPointer();
    const host = hostElement();
    mountOrgDocument(host, {
      content: { source: "see [[id:abc-123][the algorithm]]\n" },
    });
    const link = documentOf(host).querySelector("a.org-link") as HTMLAnchorElement;

    expect(link.hasAttribute("href")).toBe(false);
    expect(link.getAttribute("role")).toBe("link");
    expect(() => {
      link.dispatchEvent(new MouseEvent("mouseenter"));
      link.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, detail: 1 }));
    }).not.toThrow();
  });

  it("escapes markup and script the source carries", () => {
    const host = hostElement();
    mountOrgDocument(host, {
      content: {
        source: 'A <script>window.__breached = true</script> and <b>bold</b> literal.\n',
      },
    });

    const rendered = documentOf(host);
    expect(rendered.querySelector("script")).toBeNull();
    expect(rendered.querySelector("b")).toBeNull();
    expect(rendered.textContent).toContain("<script>window.__breached = true</script>");
    expect((window as unknown as Record<string, unknown>)["__breached"]).toBeUndefined();
  });

  it("leaves a target the document cannot follow inert", () => {
    const host = hostElement();
    mountOrgDocument(host, {
      content: {
        source: "see [[javascript:alert(1)][the paper]] and [[file:notes/a.org][a file]]\n",
      },
    });

    const rendered = documentOf(host);
    expect(rendered.querySelectorAll("a")).toHaveLength(0);
    expect(rendered.querySelectorAll(".org-link--inert")).toHaveLength(2);
  });

  it("anchors a target the host resolved and refuses a hostile resolution", () => {
    const host = hostElement();
    const handle = mountOrgDocument(host, {
      content: { source: "see [[file:media/plot.png][the plot]]\n" },
      resolveAsset: (target) => (target === "file:media/plot.png" ? "media/plot.png" : null),
    });

    const asset = documentOf(host).querySelector("a.org-link--asset") as HTMLAnchorElement;
    expect(asset.getAttribute("href")).toBe("media/plot.png");

    handle.update({ resolveAsset: () => "javascript:alert(1)" });
    expect(documentOf(host).querySelector("a")).toBeNull();
    expect(documentOf(host).querySelector(".org-link--inert")).not.toBeNull();

    handle.update({ resolveAsset: () => "//example.org/plot.png" });
    expect(documentOf(host).querySelector("a")).toBeNull();
  });

  it("renders TeX that defeats KaTeX as raw source, keeping the prose around it", () => {
    const runaway = "\\sqrt{".repeat(2000);
    const host = hostElement();
    mountOrgDocument(host, {
      content: { source: `broken \\(\\frac{1\\) and \\(${runaway}\\) here\n` },
    });

    const errors = documentOf(host).querySelectorAll(".org-math .katex-error");
    expect(errors).toHaveLength(2);
    expect(errors[0]?.textContent).toBe("\\frac{1");
    expect(errors[1]?.textContent).toBe(runaway);
    expect(documentOf(host).textContent).toContain("here");
  });

  it("reads no storage and opens no transport across a whole lifecycle", () => {
    const fetchSpy = vi.fn();
    const storage = { getItem: vi.fn(), setItem: vi.fn(), removeItem: vi.fn(), clear: vi.fn() };
    const transports = {
      XMLHttpRequest: vi.fn(),
      WebSocket: vi.fn(),
      EventSource: vi.fn(),
    };
    vi.stubGlobal("fetch", fetchSpy);
    vi.stubGlobal("localStorage", storage);
    vi.stubGlobal("sessionStorage", storage);
    for (const [name, stub] of Object.entries(transports)) {
      vi.stubGlobal(name, stub);
    }

    const host = hostElement();
    const handle = mountOrgDocument(host, {
      content: { source: "A [[id:abc-123][link]] and \\(x^2\\).\n" },
      theme: "dark",
    });
    handle.update({ content: { source: "Another line.\n" }, theme: "light" });
    handle.dispose();

    expect(fetchSpy).not.toHaveBeenCalled();
    expect(storage.getItem).not.toHaveBeenCalled();
    expect(storage.setItem).not.toHaveBeenCalled();
    for (const stub of Object.values(transports)) {
      expect(stub).not.toHaveBeenCalled();
    }
  });
});
