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

function requestFor(
  target: string,
  verbs: Partial<
    Pick<GlanceRequest, "gesture" | "origin" | "pin" | "go" | "dismiss">
  > = {},
): GlanceRequest {
  return {
    target: { id: null, target },
    origin: document.createElement("a"),
    gesture: "hover",
    pin: () => {},
    go: () => {},
    dismiss: () => {},
    ...verbs,
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

    const { container } = render(() => (
      <GlancePreview request={requestFor("notes/gradient.org")} />
    ));

    expect(await screen.findByText("Gradient descent")).toBeInTheDocument();
    expect(screen.getByText("Click to open · Alt-click to replace")).toBeInTheDocument();

    expect(container.querySelectorAll("button")).toHaveLength(0);
    expect(container.querySelector(".glance-card")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
  });

  it("announces a focus-raised card as the description of its link", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(noteContextResponse("Duality gap", "The gap closes.")),
      ),
    );
    // The link the reader is standing on, in the document so the relation between
    // the two elements is the one an assistive technology would resolve.
    const link = document.createElement("a");
    document.body.append(link);

    const { container, unmount } = render(() => (
      <GlancePreview
        request={requestFor("notes/duality.org", { gesture: "focus", origin: link })}
      />
    ));

    const card = container.querySelector(".glance-card")!;
    expect(card.id).not.toBe("");
    expect(link).toHaveAttribute("aria-describedby", card.id);
    expect(card).not.toHaveAttribute("aria-hidden");
    // Read from the link rather than stepped into: the card is no tab stop.
    expect(card).not.toHaveAttribute("tabindex");

    expect(await screen.findByText("Duality gap")).toBeInTheDocument();
    expect(link).toHaveAttribute("aria-describedby", card.id);

    // The description names an element that leaves with the card.
    unmount();
    expect(link).not.toHaveAttribute("aria-describedby");
    link.remove();
  });

  it("leaves a cursor-raised card out of what its link is announced as", () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(noteContextResponse("Duality gap", "The gap closes.")),
      ),
    );
    const link = document.createElement("a");
    document.body.append(link);

    const { container } = render(() => (
      <GlancePreview request={requestFor("notes/duality.org", { origin: link })} />
    ));

    expect(link).not.toHaveAttribute("aria-describedby");
    expect(container.querySelector(".glance-card")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
    link.remove();
  });

  it("offers the committing verbs as controls when a tap raised it", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(noteContextResponse("Duality gap", "The gap closes.")),
      ),
    );
    const pin = vi.fn();
    const go = vi.fn();
    const dismiss = vi.fn();

    const { container } = render(() => (
      <GlancePreview
        request={requestFor("notes/duality.org", {
          gesture: "touch",
          pin,
          go,
          dismiss,
        })}
      />
    ));

    expect(await screen.findByText("Duality gap")).toBeInTheDocument();
    expect(
      screen.queryByText("Click to open · Alt-click to replace"),
    ).not.toBeInTheDocument();

    const card = container.querySelector(".glance-card");
    expect(card).not.toHaveAttribute("aria-hidden");
    expect(card?.classList.contains("glance-card--committing")).toBe(true);

    screen.getByRole("button", { name: "Open" }).click();
    expect(pin).toHaveBeenCalledTimes(1);
    screen.getByRole("button", { name: "Replace" }).click();
    expect(go).toHaveBeenCalledTimes(1);
    screen.getByRole("button", { name: "Close" }).click();
    expect(dismiss).toHaveBeenCalledTimes(1);
  });

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
