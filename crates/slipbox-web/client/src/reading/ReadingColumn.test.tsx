import { render, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { __resetRefocusForTests } from "../data/refetch-on-focus.js";
import { ReadingColumn } from "./ReadingColumn.jsx";
import type { ColumnState } from "./spine-geometry.js";

function noteContextResponse(title: string, content = ""): Response {
  return new Response(
    JSON.stringify({
      note: { title, node_key: "notes/gradient.org" },
      source: { content },
      node_start_line: 1,
      node_line_count: 1,
      backlinks: [],
      forward_links: [],
    }),
    { status: 200, headers: { "content-type": "application/json" } },
  );
}

/**
 * `shown` lines served out of a note `lines` long, flagged after only, since a
 * node read begins at the note's own first line.
 */
function truncatedContextResponse(shown: number, lines: number): Response {
  return new Response(
    JSON.stringify({
      note: { title: "Gradient descent", node_key: "notes/gradient.org" },
      source: {
        content: "It goes on.",
        line_count: shown,
        // Longer than the note, as it is whenever a note is a heading.
        total_lines: lines + 2500,
        truncated_before: false,
        truncated_after: true,
      },
      node_start_line: 1,
      node_line_count: lines,
      backlinks: [],
      forward_links: [],
    }),
    { status: 200, headers: { "content-type": "application/json" } },
  );
}

function errorResponse(status: number, kind: string, message: string): Response {
  return new Response(JSON.stringify({ error: { kind, message } }), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function mount(reference: string) {
  return render(() => (
    <ReadingColumn
      reference={reference}
      state="resting"
      navigation={{ glance: () => {}, pin: () => {}, go: () => {} }}
      onReveal={() => {}}
    />
  ));
}

describe("ReadingColumn obscured sliver", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("collapses to a button that reveals its column on click", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(noteContextResponse("Gradient descent"))),
    );
    const onReveal = vi.fn();

    render(() => (
      <ReadingColumn
        reference="notes/gradient.org"
        state="obscured"
        navigation={{ glance: () => {}, pin: () => {}, go: () => {} }}
        onReveal={onReveal}
      />
    ));

    const button = await screen.findByRole("button", { name: "Reveal Gradient descent" });
    expect(button.querySelector(".reading-note__sliver-label")?.textContent).toBe(
      "Gradient descent",
    );

    button.click();
    expect(onReveal).toHaveBeenCalledTimes(1);
  });

  it("hides the note it collapses rather than discarding it", async () => {
    const read = vi.fn(() =>
      Promise.resolve(noteContextResponse("Gradient descent", "All of it.")),
    );
    vi.stubGlobal("fetch", read);
    const [state, setState] = createSignal<ColumnState>("resting");

    render(() => (
      <ReadingColumn
        reference="notes/gradient.org"
        state={state()}
        navigation={{ glance: () => {}, pin: () => {}, go: () => {} }}
        onReveal={() => {}}
      />
    ));

    expect(await screen.findByText("All of it.")).toBeInTheDocument();
    expect(read).toHaveBeenCalledTimes(1);

    // A hidden column is out of the accessibility tree, which is what a role
    // query walks, so the note reads as gone while its subtree stays mounted.
    setState("obscured");
    expect(
      screen.queryByRole("heading", { level: 1, name: "Gradient descent" }),
    ).not.toBeInTheDocument();

    setState("resting");
    expect(
      screen.getByRole("heading", { level: 1, name: "Gradient descent" }),
    ).toBeInTheDocument();
    // Still the one read: the column was hidden and shown again, not discarded
    // and fetched a second time.
    expect(read).toHaveBeenCalledTimes(1);
  });
});

describe("ReadingColumn truncated body", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("names the extent it holds against the note's own length", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(truncatedContextResponse(1000, 1840))),
    );

    mount("notes/gradient.org");

    const notice = await screen.findByText(
      "Showing 1000 of this note's 1840 lines.",
    );
    expect(notice).toHaveClass("reading-note__truncated");
    expect(
      notice.compareDocumentPosition(screen.getByText("It goes on.")) &
        Node.DOCUMENT_POSITION_PRECEDING,
    ).toBeTruthy();
  });

  it("says nothing about extent when the slice is the whole note", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(noteContextResponse("Gradient descent", "All of it.")),
      ),
    );

    mount("notes/gradient.org");

    expect(await screen.findByText("All of it.")).toBeInTheDocument();
    expect(document.querySelector(".reading-note__truncated")).toBeNull();
  });

  it("makes no claim about extent when the read failed", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(errorResponse(503, "unavailable", "daemon is down")),
      ),
    );

    mount("notes/gradient.org");

    expect(await screen.findByText("daemon is down")).toBeInTheDocument();
    expect(document.querySelector(".reading-note__truncated")).toBeNull();
  });
});

describe("ReadingColumn failure", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("offers a search seeded from the name a missing note's key carries", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(errorResponse(404, "not-found", "no note for the given key")),
      ),
    );

    mount("file:gradient-descent.org");

    expect(
      await screen.findByText("This note is not in the slipbox."),
    ).toBeInTheDocument();
    expect(screen.getByText("file:gradient-descent.org")).toBeInTheDocument();
    const search = screen.getByRole("link", {
      name: "Search the slipbox for gradient descent",
    });
    expect(search).toHaveAttribute("href", "?q=gradient+descent");
  });

  it("offers no search for an id reference, which names nothing to search for", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(errorResponse(404, "not-found", "no note matched the given selector")),
      ),
    );

    mount("id:6f1c2e9a-0000-4000-8000-000000000000");

    expect(
      await screen.findByText("This note is not in the slipbox."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Try reading it again" }),
    ).toBeInTheDocument();
  });

  it("reads the note again on request, and shows it once the read succeeds", async () => {
    let reachable = false;
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          reachable
            ? noteContextResponse("Gradient descent", "It reads now.")
            : errorResponse(503, "unavailable", "daemon is down"),
        ),
      ),
    );

    mount("notes/gradient.org");

    expect(await screen.findByText("daemon is down")).toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();

    reachable = true;
    screen.getByRole("button", { name: "Try reading it again" }).click();

    expect(await screen.findByText("It reads now.")).toBeInTheDocument();
    expect(screen.queryByText("daemon is down")).not.toBeInTheDocument();
  });
});

describe("ReadingColumn document semantics", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("names a column for the note it holds", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(noteContextResponse("Gradient descent", "All of it.")),
      ),
    );

    mount("notes/gradient.org");

    expect(await screen.findByText("All of it.")).toBeInTheDocument();
    const heading = screen.getByRole("heading", {
      level: 1,
      name: "Gradient descent",
    });
    expect(
      screen.getByRole("article", { name: "Gradient descent" }),
    ).toContainElement(heading);
  });

  it("heads a recovery column with the failure and names it for that", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(errorResponse(404, "not-found", "no note for the given key")),
      ),
    );

    mount("file:gradient-descent.org");

    const heading = await screen.findByRole("heading", {
      level: 1,
      name: "This note is not in the slipbox.",
    });
    expect(heading).toHaveClass("reading-note__status--error");
    expect(
      screen.getByRole("article", { name: "This note is not in the slipbox." }),
    ).toContainElement(heading);
  });

  it("announces the failure politely", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(errorResponse(503, "unavailable", "daemon is down")),
      ),
    );

    mount("notes/gradient.org");

    expect(await screen.findByText("daemon is down")).toHaveAttribute(
      "aria-live",
      "polite",
    );
  });

  it("lands the failure in the region that was already reading", async () => {
    let answer: (response: Response) => void = () => {};
    vi.stubGlobal(
      "fetch",
      vi.fn(
        () => new Promise<Response>((resolve) => {
          answer = resolve;
        }),
      ),
    );

    mount("notes/gradient.org");

    const region = await screen.findByText("Reading…");
    expect(region).toHaveAttribute("aria-live", "polite");

    answer(errorResponse(503, "unavailable", "daemon is down"));

    expect(await screen.findByText("daemon is down")).toBe(region);
  });

  it("names a column that is still reading for what it is doing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => new Promise<Response>(() => {})),
    );

    mount("notes/gradient.org");

    const heading = await screen.findByRole("heading", {
      level: 1,
      name: "Reading…",
    });
    expect(
      screen.getByRole("article", { name: "Reading…" }),
    ).toContainElement(heading);
  });

  it("keeps both ways out inside the column it names, and working", async () => {
    let reachable = false;
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          reachable
            ? noteContextResponse("Gradient descent", "It reads now.")
            : errorResponse(404, "not-found", "no note for the given key"),
        ),
      ),
    );

    mount("file:gradient-descent.org");

    await screen.findByRole("heading", {
      level: 1,
      name: "This note is not in the slipbox.",
    });
    const column = screen.getByRole("article", {
      name: "This note is not in the slipbox.",
    });
    const retry = screen.getByRole("button", { name: "Try reading it again" });
    expect(column).toContainElement(retry);
    expect(column).toContainElement(
      screen.getByRole("link", {
        name: "Search the slipbox for gradient descent",
      }),
    );

    reachable = true;
    retry.click();

    expect(await screen.findByText("It reads now.")).toBeInTheDocument();
    expect(
      screen.getByRole("article", { name: "Gradient descent" }),
    ).toBeInTheDocument();
  });
});
