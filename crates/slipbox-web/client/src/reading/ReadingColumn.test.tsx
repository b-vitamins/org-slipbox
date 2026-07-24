import { render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { __resetRefocusForTests } from "../data/refetch-on-focus.js";
import { ReadingColumn } from "./ReadingColumn.jsx";

/** A note-context response carrying just the title the sliver reads. */
function noteContextResponse(title: string): Response {
  return new Response(
    JSON.stringify({
      note: { title },
      source: { content: "" },
      node_start_line: 1,
      node_line_count: 1,
      backlinks: [],
      forward_links: [],
    }),
    { status: 200, headers: { "content-type": "application/json" } },
  );
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
});
