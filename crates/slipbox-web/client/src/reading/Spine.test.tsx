import { render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { __resetRefocusForTests } from "../data/refetch-on-focus.js";
import { Spine } from "./Spine.jsx";
import { createReadingStack } from "./stack.js";

function noteContextResponse(
  title: string,
  content = "See [[id:abc-123][the other note]].",
): Response {
  return new Response(
    JSON.stringify({
      note: { title, node_key: "notes/one.org" },
      source: { content },
      node_start_line: 1,
      node_line_count: 1,
      backlinks: [],
      forward_links: [],
    }),
    { status: 200, headers: { "content-type": "application/json" } },
  );
}

/** A 200 whose body omits the source the renderer reads: a contract drift. */
function bodylessResponse(title: string, key: string): Response {
  return new Response(
    JSON.stringify({
      note: { title, node_key: key },
      node_start_line: 1,
      node_line_count: 1,
      backlinks: [],
      forward_links: [],
    }),
    { status: 200, headers: { "content-type": "application/json" } },
  );
}

function stubResponsiveFrame(): {
  setNarrow: (value: boolean) => void;
  restore: () => void;
} {
  let narrow = false;
  let scrollLeft = 0;
  let scrollTop = 0;
  const descriptors = new Map<string, PropertyDescriptor | undefined>();
  for (const [name, descriptor] of Object.entries({
    clientWidth: { configurable: true, get: () => (narrow ? 800 : 801) },
    scrollWidth: { configurable: true, get: () => (narrow ? 800 : 3 * 625) },
    scrollLeft: {
      configurable: true,
      get: () => scrollLeft,
      set: (value: number) => {
        scrollLeft = value;
      },
    },
    scrollTop: {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => {
        scrollTop = value;
      },
    },
  })) {
    descriptors.set(name, Object.getOwnPropertyDescriptor(HTMLElement.prototype, name));
    Object.defineProperty(HTMLElement.prototype, name, descriptor);
  }

  const realStyle = globalThis.getComputedStyle;
  vi.stubGlobal("getComputedStyle", (element: Element) => {
    const style = realStyle(element);
    return new Proxy(style, {
      get(target, property) {
        if (property === "flexDirection" && element.classList.contains("spine")) {
          return narrow ? "column" : "row";
        }
        const value = Reflect.get(target, property, target);
        return typeof value === "function" ? value.bind(target) : value;
      },
    });
  });

  const rect = vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockImplementation(function (this: HTMLElement) {
      if (this.classList.contains("spine")) {
        return DOMRect.fromRect({ x: 0, y: 50, width: narrow ? 800 : 801, height: 700 });
      }
      if (this.classList.contains("spine-column")) {
        const columns = [...this.parentElement!.querySelectorAll(".spine-column")];
        const index = columns.indexOf(this);
        return DOMRect.fromRect({
          x: 0,
          y: 50 + index * 700 - scrollTop,
          width: narrow ? 800 : 625,
          height: 700,
        });
      }
      return DOMRect.fromRect();
    });

  return {
    setNarrow(value) {
      narrow = value;
    },
    restore() {
      rect.mockRestore();
      for (const [name, descriptor] of descriptors) {
        if (descriptor === undefined) {
          Reflect.deleteProperty(HTMLElement.prototype, name);
        } else {
          Object.defineProperty(HTMLElement.prototype, name, descriptor);
        }
      }
    },
  };
}

describe("Spine", () => {
  beforeEach(() => {
    __resetRefocusForTests();
    // jsdom has no layout and no `Element.scrollTo`; the spine calls it when it
    // reveals a column.
    Element.prototype.scrollTo = vi.fn();
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string) =>
        Promise.resolve(
          input.includes("/api/node")
            ? new Response(
                JSON.stringify({ node_key: "notes/one.org", title: "One" }),
                { status: 200, headers: { "content-type": "application/json" } },
              )
            : noteContextResponse("One"),
        ),
      ),
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("dismisses an open preview when a column scrolls its own body", async () => {
    const stack = createReadingStack({
      read: () => "?note=notes/one.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    const link = await screen.findByRole("link", { name: "the other note" });
    link.dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));
    link.dispatchEvent(new MouseEvent("mouseenter"));
    await vi.waitFor(() =>
      expect(container.querySelector(".glance-card")).not.toBeNull(),
    );

    const column = container.querySelector(".spine-column");
    expect(column).not.toBeNull();
    // `scroll` from a descendant does not bubble, exactly as the browser fires it.
    column!.dispatchEvent(new Event("scroll"));
    expect(container.querySelector(".glance-card")).toBeNull();
  });

  it("reports a note it cannot render instead of reading it forever", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string) => {
        if (input.includes("/api/node")) {
          return Promise.resolve(
            new Response(
              JSON.stringify({ node_key: "notes/one.org", title: "One" }),
              { status: 200, headers: { "content-type": "application/json" } },
            ),
          );
        }
        // The second column's note answers 200 with no source to render.
        return Promise.resolve(
          input.includes("notes%2Ftwo.org")
            ? bodylessResponse("Two", "notes/two.org")
            : noteContextResponse("One"),
        );
      }),
    );
    const stack = createReadingStack({
      read: () => "?note=notes/one.org&stacked=notes/two.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    expect(
      await screen.findByText("This note could not be rendered."),
    ).toBeInTheDocument();
    expect(await screen.findByText("One")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "the other note" }),
    ).toBeInTheDocument();
    expect(container.querySelectorAll(".spine-column")).toHaveLength(2);
  });

  it("reads a note again after a retry, once it can be drawn", async () => {
    let drawable = false;
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string) => {
        if (input.includes("/api/node")) {
          return Promise.resolve(
            new Response(
              JSON.stringify({ node_key: "notes/two.org", title: "Two" }),
              { status: 200, headers: { "content-type": "application/json" } },
            ),
          );
        }
        // The note answers 200 with no source to render until it is repaired.
        return Promise.resolve(
          drawable
            ? noteContextResponse("Two", "Now it reads.")
            : bodylessResponse("Two", "notes/two.org"),
        );
      }),
    );
    const stack = createReadingStack({
      read: () => "?note=notes/two.org",
      push: () => {},
      replace: () => {},
    });
    render(() => <Spine stack={stack} />);

    const retry = await screen.findByRole("button", { name: "Read it again" });

    retry.click();
    expect(
      await screen.findByRole("button", { name: "Read it again" }),
    ).toBeInTheDocument();

    drawable = true;
    screen.getByRole("button", { name: "Read it again" }).click();
    expect(await screen.findByText("Now it reads.")).toBeInTheDocument();
    expect(
      screen.queryByText("This note could not be rendered."),
    ).not.toBeInTheDocument();
  });

  it("drops a preview it cannot draw without disabling the next one", async () => {
    // The undrawable preview's fetch parks until released, which holds the card
    // in its open, still-reading state long enough to observe.
    let release: (() => void) | null = null;
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string) => {
        // A glance resolves its `id:` target to a node first, so each id answers
        // with its own key for the context fetch to be distinguishable.
        if (input.includes("/api/node?id=")) {
          const key = input.includes("broken")
            ? "notes/broken.org"
            : "notes/fine.org";
          return Promise.resolve(
            new Response(JSON.stringify({ node_key: key, title: key }), {
              status: 200,
              headers: { "content-type": "application/json" },
            }),
          );
        }
        if (input.includes("/api/node")) {
          return Promise.resolve(
            new Response(
              JSON.stringify({ node_key: "notes/one.org", title: "One" }),
              { status: 200, headers: { "content-type": "application/json" } },
            ),
          );
        }
        // The first link's target answers 200 with no source to render.
        if (input.includes("notes%2Fbroken.org")) {
          return new Promise<Response>((resolve) => {
            release = () =>
              resolve(bodylessResponse("Broken", "notes/broken.org"));
          });
        }
        return Promise.resolve(
          input.includes("notes%2Ffine.org")
            ? noteContextResponse("Fine")
            : noteContextResponse(
                "One",
                "See [[id:broken][bad link]] and [[id:fine][good link]].",
              ),
        );
      }),
    );
    const stack = createReadingStack({
      read: () => "?note=notes/one.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    const bad = await screen.findByRole("link", { name: "bad link" });
    bad.dispatchEvent(new MouseEvent("mouseenter"));
    await vi.waitFor(() => expect(release).not.toBeNull());
    expect(screen.getByText("Reading…")).toBeInTheDocument();

    release!();
    await vi.waitFor(() =>
      expect(container.querySelector(".glance-card")).toBeNull(),
    );
    expect(screen.getByRole("link", { name: "bad link" })).toBeInTheDocument();

    // No dismissal in between, so this is the in-place swap of an open card.
    screen
      .getByRole("link", { name: "good link" })
      .dispatchEvent(new MouseEvent("mouseenter"));

    expect(await screen.findByText("Fine")).toBeInTheDocument();
    expect(container.querySelector(".glance-card")).not.toBeNull();
  });

  it("keeps the reading position across the 800px layout boundary", async () => {
    const frame = stubResponsiveFrame();
    try {
      const stack = createReadingStack({
        read: () =>
          "?note=notes/one.org&stacked=notes/two.org&stacked=notes/three.org",
        push: () => {},
        replace: () => {},
      });
      const { container } = render(() => <Spine stack={stack} />);
      const spine = container.querySelector<HTMLElement>(".spine")!;

      spine.scrollLeft = 585;
      spine.dispatchEvent(new Event("scroll"));
      expect(screen.getByText("Note 2 of 3")).toHaveAttribute(
        "aria-current",
        "step",
      );

      vi.mocked(Element.prototype.scrollTo).mockClear();
      frame.setNarrow(true);
      spine.scrollLeft = 0;
      spine.dispatchEvent(new Event("scroll"));
      window.dispatchEvent(new Event("resize"));
      await vi.waitFor(() =>
        expect(Element.prototype.scrollTo).toHaveBeenCalledWith(
          expect.objectContaining({ top: 700 }),
        ),
      );

      spine.scrollTop = 700;
      spine.dispatchEvent(new Event("scroll"));
      vi.mocked(Element.prototype.scrollTo).mockClear();
      frame.setNarrow(false);
      spine.scrollTop = 0;
      spine.dispatchEvent(new Event("scroll"));
      window.dispatchEvent(new Event("resize"));
      await vi.waitFor(() =>
        expect(Element.prototype.scrollTo).toHaveBeenCalledWith(
          expect.objectContaining({ left: 537 }),
        ),
      );
      expect(screen.getByText("Note 2 of 3")).toHaveAttribute(
        "aria-current",
        "step",
      );
    } finally {
      frame.restore();
    }
  });

  it("exposes the stack position and adjacent-note controls", () => {
    const stack = createReadingStack({
      read: () => "?note=notes/one.org&stacked=notes/two.org",
      push: () => {},
      replace: () => {},
    });
    render(() => <Spine stack={stack} />);

    expect(screen.getAllByRole("navigation", { name: "Reading trail" })).toHaveLength(
      2,
    );
    expect(screen.getByText("Note 1 of 2")).toBeInTheDocument();
    expect(screen.getByText("Note 2 of 2")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Previous" })[0]).toBeDisabled();
    expect(screen.getAllByRole("button", { name: "Next" })[1]).toBeDisabled();
  });

  it("takes focus into the column it opens, named for that note", async () => {
    const stack = createReadingStack({
      read: () => "?note=notes/one.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    const column = container.querySelector<HTMLElement>("article.reading-note");
    expect(column).not.toBeNull();
    await vi.waitFor(() => expect(column).toHaveFocus());

    // What the focused element is called is the note's own heading, so a reader
    // is told which note opened rather than that something did.
    const heading = await screen.findByRole("heading", { level: 1, name: "One" });
    expect(column!.getAttribute("aria-labelledby")).toBe(heading.id);
  });

  it("takes focus into the column a pinned link opens", async () => {
    const stack = createReadingStack({
      read: () => "?note=notes/one.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    // `click()` synthesizes a click of no pointer press, which the grammar reads
    // as the keyboard activation it is, and pins.
    (await screen.findByRole("link", { name: "the other note" })).click();

    await vi.waitFor(() =>
      expect(container.querySelectorAll(".spine-column")).toHaveLength(2),
    );
    const opened = container.querySelectorAll<HTMLElement>(".spine-column")[1]!;
    await vi.waitFor(() =>
      expect(opened.contains(document.activeElement)).toBe(true),
    );
  });

  it("leaves focus alone when a re-render finds the stack unchanged", async () => {
    const stack = createReadingStack({
      read: () => "?note=notes/one.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    const column = container.querySelector<HTMLElement>("article.reading-note")!;
    await vi.waitFor(() => expect(column).toHaveFocus());

    // Focus has since left the spine for the header, and a re-measure re-renders
    // every column: the reveal is what moves focus, not the render.
    column.blur();
    expect(document.body).toHaveFocus();
    window.dispatchEvent(new Event("resize"));
    await Promise.resolve();

    expect(document.body).toHaveFocus();
  });
});
