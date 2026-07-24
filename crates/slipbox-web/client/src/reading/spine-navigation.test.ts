import { describe, expect, it, vi } from "vitest";

import type { GlanceController } from "./glance-controller.js";
import { spineNavigation } from "./spine-navigation.js";
import type { ReadingStack } from "./stack.js";
import type { GlanceRequest, LinkTarget } from "../org/navigation.jsx";

/**
 * A stack whose methods are spies, so wiring can be asserted directly. `follow`
 * reports the column the target landed in; `landsAt` sets which.
 */
function stubStack(landsAt = 0): ReadingStack & {
  follow: ReturnType<typeof vi.fn>;
  open: ReturnType<typeof vi.fn>;
} {
  return {
    keys: () => [],
    follow: vi.fn(() => landsAt),
    open: vi.fn(),
    sync: vi.fn(),
  };
}

function stubGlances(): GlanceController & { glance: ReturnType<typeof vi.fn> } {
  return {
    request: () => null,
    glance: vi.fn(),
    cancel: vi.fn(),
  };
}

const TARGET: LinkTarget = { id: "abc-123", target: "id:abc-123" };

describe("spineNavigation", () => {
  it("pins a target as a follow from the column's live index", () => {
    const stack = stubStack();
    const glances = stubGlances();
    const nav = spineNavigation(stack, glances, () => 2, vi.fn());

    nav.pin(TARGET);

    expect(stack.follow).toHaveBeenCalledWith(2, "id:abc-123");
    expect(stack.open).not.toHaveBeenCalled();
    // Pinning dismisses any open preview.
    expect(glances.glance).toHaveBeenCalledWith(null);
  });

  it("reads the index at call time, not at binding time", () => {
    const stack = stubStack();
    let index = 0;
    const nav = spineNavigation(stack, stubGlances(), () => index, vi.fn());

    index = 3;
    nav.pin(TARGET);

    expect(stack.follow).toHaveBeenCalledWith(3, "id:abc-123");
  });

  // Whether the pin opened a column or revealed one already open, the reader is
  // taken to the column the stack reports.
  it("reveals the column the follow landed in", () => {
    const reveal = vi.fn();
    const nav = spineNavigation(stubStack(0), stubGlances(), () => 2, reveal);

    nav.pin(TARGET);

    expect(reveal).toHaveBeenCalledWith(0);
  });

  it("goes to a target by replacing the whole path", () => {
    const stack = stubStack();
    const glances = stubGlances();
    const nav = spineNavigation(stack, glances, () => 1, vi.fn());

    nav.go(TARGET);

    expect(stack.open).toHaveBeenCalledWith("id:abc-123");
    expect(stack.follow).not.toHaveBeenCalled();
    expect(glances.glance).toHaveBeenCalledWith(null);
  });

  it("delegates glance straight through to the controller", () => {
    const glances = stubGlances();
    const nav = spineNavigation(stubStack(), glances, () => 0, vi.fn());
    const request = { target: TARGET, origin: {} as HTMLElement } as GlanceRequest;

    nav.glance(request);
    expect(glances.glance).toHaveBeenCalledWith(request);
  });

  it("passes a raw key target through verbatim when there is no id", () => {
    const stack = stubStack();
    const nav = spineNavigation(stack, stubGlances(), () => 0, vi.fn());

    nav.pin({ id: null, target: "heading:notes/a.org:5" });
    expect(stack.follow).toHaveBeenCalledWith(0, "heading:notes/a.org:5");
  });
});
