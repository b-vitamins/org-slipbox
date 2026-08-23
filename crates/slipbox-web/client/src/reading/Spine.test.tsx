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

function stubSpineFrame(viewport: number, scrollWidth: number): () => void {
  let scrollLeft = 0;
  const stubs: Record<string, PropertyDescriptor> = {
    clientWidth: { configurable: true, get: () => viewport },
    scrollWidth: { configurable: true, get: () => scrollWidth },
    scrollLeft: {
      configurable: true,
      get: () => scrollLeft,
      set: (value: number) => {
        scrollLeft = value;
      },
    },
  };
  for (const [name, descriptor] of Object.entries(stubs)) {
    Object.defineProperty(HTMLElement.prototype, name, descriptor);
  }
  return () => {
    for (const name of Object.keys(stubs)) {
      Reflect.deleteProperty(HTMLElement.prototype, name);
    }
  };
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

  it("leaves Escape to the surface once a scroll has dropped the card", async () => {
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

    container.querySelector(".spine-column")!.dispatchEvent(new Event("scroll"));
    expect(container.querySelector(".glance-card")).toBeNull();

    const escape = link.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Escape",
        bubbles: true,
        cancelable: true,
      }),
    );
    expect(escape).toBe(true);
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

    const heading = await screen.findByRole("heading", { level: 1, name: "One" });
    expect(column!.getAttribute("aria-labelledby")).toBe(heading.id);
  });

  it("holds focus out of the column it opens until the note is read", async () => {
    let release: ((response: Response) => void) | null = null;
    vi.stubGlobal(
      "fetch",
      vi.fn(
        () =>
          new Promise<Response>((resolve) => {
            release = resolve;
          }),
      ),
    );
    const stack = createReadingStack({
      read: () => "?note=notes/one.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    const column = container.querySelector<HTMLElement>("article.reading-note")!;
    await vi.waitFor(() => expect(release).not.toBeNull());
    expect(screen.getByText("Reading…")).toBeInTheDocument();
    expect(column).not.toHaveFocus();

    release!(noteContextResponse("One"));
    await vi.waitFor(() => expect(column).toHaveFocus());
    const heading = await screen.findByRole("heading", { level: 1, name: "One" });
    expect(column.getAttribute("aria-labelledby")).toBe(heading.id);
  });

  it("takes focus into a column that could not read its note", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(
            JSON.stringify({ error: { kind: "not-found", message: "no note" } }),
            { status: 404, headers: { "content-type": "application/json" } },
          ),
        ),
      ),
    );
    const stack = createReadingStack({
      read: () => "?note=notes/gone.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    const column = container.querySelector<HTMLElement>("article.reading-note")!;
    await vi.waitFor(() => expect(column).toHaveFocus());
    const named = document.getElementById(column.getAttribute("aria-labelledby")!);
    expect(named).toHaveTextContent("This note is not in the slipbox.");
  });

  it("takes focus into a column whose note could not be drawn", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(bodylessResponse("One", "notes/one.org"))),
    );
    const stack = createReadingStack({
      read: () => "?note=notes/one.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    await screen.findByText("This note could not be rendered.");
    const column = container.querySelector<HTMLElement>("article.reading-note")!;
    await vi.waitFor(() => expect(column).toHaveFocus());
    const named = document.getElementById(column.getAttribute("aria-labelledby")!);
    expect(named).toHaveTextContent("This note could not be rendered.");
    expect(document.title).toContain("Unavailable");
  });

  it("takes focus into the column a pinned link opens", async () => {
    const stack = createReadingStack({
      read: () => "?note=notes/one.org",
      push: () => {},
      replace: () => {},
    });
    const { container } = render(() => <Spine stack={stack} />);

    (await screen.findByRole("link", { name: "the other note" })).click();

    await vi.waitFor(() =>
      expect(container.querySelectorAll(".spine-column")).toHaveLength(2),
    );
    const opened = container.querySelectorAll<HTMLElement>(".spine-column")[1]!;
    await vi.waitFor(() =>
      expect(opened.contains(document.activeElement)).toBe(true),
    );
  });

  it("names the tab after the column it reads, asking for no title of its own", async () => {
    const stack = createReadingStack({
      read: () => "?note=notes/one.org",
      push: () => {},
      replace: () => {},
    });
    render(() => <Spine stack={stack} />);

    await screen.findByRole("heading", { level: 1, name: "One" });
    await vi.waitFor(() => expect(document.title).toBe("One — slipbox"));
    const identityReads = vi
      .mocked(fetch)
      .mock.calls.filter(([input]) => String(input).startsWith("/api/node?"));
    expect(identityReads).toHaveLength(0);
  });

  it("names the tab after the failure when the column cannot be read", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(
            JSON.stringify({ error: { kind: "not-found", message: "no note" } }),
            { status: 404, headers: { "content-type": "application/json" } },
          ),
        ),
      ),
    );
    const stack = createReadingStack({
      read: () => "?note=notes/gone.org",
      push: () => {},
      replace: () => {},
    });
    render(() => <Spine stack={stack} />);

    await vi.waitFor(() =>
      expect(document.title).toBe("Note not found — slipbox"),
    );
  });

  it("retitles the tab to the column a scroll brings back, and fetches nothing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string) =>
        Promise.resolve(
          noteContextResponse(
            input.includes("three") ? "Three" : input.includes("two") ? "Two" : "One",
          ),
        ),
      ),
    );
    const push = vi.fn();
    const replace = vi.fn();
    const restoreFrame = stubSpineFrame(1200, 3 * 625);
    try {
      const stack = createReadingStack({
        read: () =>
          "?note=notes/one.org&stacked=notes/two.org&stacked=notes/three.org",
        push,
        replace,
      });
      const { container } = render(() => <Spine stack={stack} />);
      const spine = container.querySelector<HTMLElement>(".spine")!;

      await vi.waitFor(() => expect(document.title).toBe("One — slipbox"));
      const reads = vi.mocked(fetch).mock.calls.length;

      spine.scrollLeft = 585;
      spine.dispatchEvent(new Event("scroll"));
      await vi.waitFor(() => expect(document.title).toBe("Two — slipbox"));

      spine.scrollLeft = 675;
      spine.dispatchEvent(new Event("scroll"));
      await vi.waitFor(() => expect(document.title).toBe("Three — slipbox"));

      spine.scrollLeft = 0;
      spine.dispatchEvent(new Event("scroll"));
      await vi.waitFor(() => expect(document.title).toBe("One — slipbox"));

      expect(vi.mocked(fetch).mock.calls).toHaveLength(reads);
      expect(push).not.toHaveBeenCalled();
      expect(replace).not.toHaveBeenCalled();
    } finally {
      restoreFrame();
    }
  });

  it("forgets the title of a column the stack closed", async () => {
    let park: ((response: Response) => void) | null = null;
    let readsOfTwo = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string) => {
        if (input.includes("two")) {
          readsOfTwo += 1;
          if (readsOfTwo === 1) {
            return Promise.resolve(noteContextResponse("Two"));
          }
          return new Promise<Response>((resolve) => {
            park = resolve;
          });
        }
        return Promise.resolve(
          noteContextResponse(input.includes("three") ? "Three" : "One"),
        );
      }),
    );
    let address = "?note=notes/one.org&stacked=notes/two.org";
    const stack = createReadingStack({
      read: () => address,
      push: () => {},
      replace: () => {},
    });
    render(() => <Spine stack={stack} />);

    await vi.waitFor(() => expect(document.title).toBe("Two — slipbox"));

    address = "?note=notes/one.org&stacked=notes/three.org";
    stack.sync();
    await vi.waitFor(() => expect(document.title).toBe("Three — slipbox"));

    address = "?note=notes/one.org&stacked=notes/two.org";
    stack.sync();
    await vi.waitFor(() => expect(park).not.toBeNull());
    expect(document.title).toBe("slipbox");

    park!(noteContextResponse("Two, revised"));
    await vi.waitFor(() =>
      expect(document.title).toBe("Two, revised — slipbox"),
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

    column.blur();
    expect(document.body).toHaveFocus();
    window.dispatchEvent(new Event("resize"));
    await Promise.resolve();

    expect(document.body).toHaveFocus();
  });

  it("keeps the same note active across the 800px layout boundary", async () => {
    const frame = stubResponsiveFrame();
    try {
      vi.stubGlobal(
        "fetch",
        vi.fn((input: string) =>
          Promise.resolve(
            noteContextResponse(
              input.includes("three") ? "Three" : input.includes("two") ? "Two" : "One",
            ),
          ),
        ),
      );
      const stack = createReadingStack({
        read: () =>
          "?note=notes/one.org&stacked=notes/two.org&stacked=notes/three.org",
        push: () => {},
        replace: () => {},
      });
      const { container } = render(() => <Spine stack={stack} />);
      const spine = container.querySelector<HTMLElement>(".spine")!;
      await screen.findByRole("heading", { name: "Two" });

      spine.scrollLeft = 585;
      spine.dispatchEvent(new Event("scroll"));
      await vi.waitFor(() => expect(document.title).toBe("Two — slipbox"));

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
      expect(document.title).toBe("Two — slipbox");

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
      expect(document.title).toBe("Two — slipbox");
    } finally {
      frame.restore();
    }
  });

  it("exposes the stack position and adjacent-note controls", async () => {
    const stack = createReadingStack({
      read: () => "?note=notes/one.org&stacked=notes/two.org",
      push: () => {},
      replace: () => {},
    });
    render(() => <Spine stack={stack} />);
    await screen.findAllByRole("heading", { level: 1 });

    expect(screen.getAllByRole("navigation", { name: "Reading trail" })).toHaveLength(
      2,
    );
    expect(screen.getByText("Note 1 of 2")).toBeInTheDocument();
    expect(screen.getByText("Note 2 of 2")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Previous" })[0]).toBeDisabled();
    expect(screen.getAllByRole("button", { name: "Next" })[1]).toBeDisabled();
  });
});
