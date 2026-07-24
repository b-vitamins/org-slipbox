import { afterEach, describe, expect, it, vi } from "vitest";

import { revealOption } from "./scroll-into-view.js";

/** An option element in the document, named by id the way a listbox names it. */
function option(id: string): HTMLElement {
  const element = document.createElement("li");
  element.id = id;
  document.body.append(element);
  return element;
}

describe("revealOption", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("scrolls the named option to the nearest edge", () => {
    const element = option("entry-option-3");
    const scrollIntoView = vi.fn();
    element.scrollIntoView = scrollIntoView;

    revealOption("entry-option-3");

    expect(scrollIntoView).toHaveBeenCalledWith({
      block: "nearest",
      behavior: "smooth",
    });
  });

  it("jumps instead of gliding when motion is reduced", () => {
    const element = option("entry-option-0");
    const scrollIntoView = vi.fn();
    element.scrollIntoView = scrollIntoView;
    vi.stubGlobal(
      "matchMedia",
      vi.fn((query: string) => ({ matches: true, media: query })),
    );

    revealOption("entry-option-0");

    expect(scrollIntoView).toHaveBeenCalledWith({
      block: "nearest",
      behavior: "auto",
    });
  });

  it("does nothing without a cursor", () => {
    const element = option("entry-option-0");
    const scrollIntoView = vi.fn();
    element.scrollIntoView = scrollIntoView;

    revealOption(undefined);

    expect(scrollIntoView).not.toHaveBeenCalled();
  });

  it("does nothing when the named option has left the document", () => {
    // The list can change under a cursor, leaving the id pointing at nothing.
    expect(() => revealOption("entry-option-9")).not.toThrow();
  });
});
