import { render, screen } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { __resetRefocusForTests } from "../data/refetch-on-focus.js";
import type { GlanceRequest } from "../org/navigation.jsx";
import { visibleText } from "../test/visible-text.js";
import { GlancePreview } from "./GlancePreview.jsx";

function noteContextResponse(title: string, content: string): Response {
  return new Response(
    JSON.stringify({
      note: { title },
      source: { content },
      node_start_line: 1,
      node_line_count: 1,
      backlinks: [],
      forward_links: [],
    }),
    { status: 200, headers: { "content-type": "application/json" } },
  );
}

/** A glance request anchored at a throwaway element (raw-key target). */
function requestFor(target: string): GlanceRequest {
  return {
    target: { id: null, target },
    origin: document.createElement("a"),
  };
}

describe("GlancePreview", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("surfaces the navigation grammar as a hint once the target resolves", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(noteContextResponse("Gradient descent", "A first-order method.")),
      ),
    );

    render(() => <GlancePreview request={requestFor("notes/gradient.org")} />);

    expect(await screen.findByText("Gradient descent")).toBeInTheDocument();
    expect(screen.getByText("Click to open · Alt-click to replace")).toBeInTheDocument();
  });

  // A peek at a note should look like the note: an excerpt carrying a formula
  // previews with that formula typeset, not with its TeX spelled out.
  it("typesets math in the excerpt through KaTeX", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          noteContextResponse(
            "Change of variables",
            "The density of \\(\\mathbf{x}\\) follows.",
          ),
        ),
      ),
    );

    const { container } = render(() => (
      <GlancePreview request={requestFor("notes/change.org")} />
    ));

    await screen.findByText("Change of variables");
    const excerpt = container.querySelector(".glance-card__excerpt");
    expect(excerpt?.querySelector(".katex")).not.toBeNull();
    // KaTeX keeps the TeX in its MathML annotation, so visibleText is what
    // asserts on the glyphs the reader sees rather than the source.
    expect(visibleText(excerpt)).toBe("The density of x follows.");
  });

  it("never shows the previous target's excerpt after a swap", async () => {
    const gate: (() => void)[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string) =>
        new Promise<Response>((resolve) => {
          const body = input.includes("second")
            ? noteContextResponse("Second note", "The second body.")
            : noteContextResponse("First note", "The first body.");
          gate.push(() => resolve(body));
        }),
      ),
    );

    const [request, setRequest] = createSignal(requestFor("notes/first.org"));
    const { container } = render(() => <GlancePreview request={request()} />);

    gate.shift()!();
    expect(await screen.findByText("First note")).toBeInTheDocument();
    expect(visibleText(container.querySelector(".glance-card__excerpt"))).toBe(
      "The first body.",
    );

    setRequest(requestFor("notes/second.org"));
    await Promise.resolve();
    expect(screen.queryByText("First note")).not.toBeInTheDocument();
    expect(container.querySelector(".glance-card__excerpt")).toBeNull();
    expect(screen.getByText("Reading…")).toBeInTheDocument();

    gate.shift()!();
    expect(await screen.findByText("Second note")).toBeInTheDocument();
    expect(visibleText(container.querySelector(".glance-card__excerpt"))).toBe(
      "The second body.",
    );
  });

  it("does not report a failed read as a missing note", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(
            JSON.stringify({ error: { kind: "internal", message: "index locked" } }),
            { status: 500, headers: { "content-type": "application/json" } },
          ),
        ),
      ),
    );

    render(() => <GlancePreview request={requestFor("notes/present.org")} />);

    expect(await screen.findByText("This note could not be read.")).toBeInTheDocument();
    expect(
      screen.queryByText("This note is not in the slipbox."),
    ).not.toBeInTheDocument();
  });

  it("clears a failed target's message when the next one resolves", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((input: string) =>
        Promise.resolve(
          input.includes("missing")
            ? new Response("{}", { status: 404 })
            : noteContextResponse("Present note", "A body."),
        ),
      ),
    );

    const [request, setRequest] = createSignal(requestFor("notes/missing.org"));
    render(() => <GlancePreview request={request()} />);
    expect(await screen.findByText("This note is not in the slipbox.")).toBeInTheDocument();

    setRequest(requestFor("notes/present.org"));
    expect(await screen.findByText("Present note")).toBeInTheDocument();
    expect(
      screen.queryByText("This note is not in the slipbox."),
    ).not.toBeInTheDocument();
  });
});
