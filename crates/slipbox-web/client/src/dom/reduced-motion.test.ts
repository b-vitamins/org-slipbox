import { afterEach, describe, expect, it, vi } from "vitest";

import { prefersReducedMotion, scrollBehavior } from "./reduced-motion.js";

/** Stub `window.matchMedia` so the reduce query resolves to `matches`. */
function stubMatchMedia(matches: boolean): void {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("reduce") ? matches : false,
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

describe("reduced motion", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("reports the reduce preference and jumps the scroll when it is set", () => {
    stubMatchMedia(true);
    expect(prefersReducedMotion()).toBe(true);
    expect(scrollBehavior()).toBe("auto");
  });

  it("glides the scroll when motion is not reduced", () => {
    stubMatchMedia(false);
    expect(prefersReducedMotion()).toBe(false);
    expect(scrollBehavior()).toBe("smooth");
  });
});
