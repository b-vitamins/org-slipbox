import { afterEach, describe, expect, it, vi } from "vitest";

import { gestureCanHover, pointerCanHover } from "./pointer.js";

/** Stub `window.matchMedia` so the `hover: none` query answers `matches`. */
function stubMatchMedia(matches: boolean): void {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("hover: none") ? matches : false,
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

describe("pointerCanHover", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("reports a finger as unable to rest on a link", () => {
    stubMatchMedia(true);
    expect(pointerCanHover()).toBe(false);
  });

  it("reports a cursor as able to rest on a link", () => {
    stubMatchMedia(false);
    expect(pointerCanHover()).toBe(true);
  });

  it("assumes a cursor where the platform cannot say", () => {
    vi.stubGlobal("matchMedia", undefined);
    expect(pointerCanHover()).toBe(true);
  });
});

describe("gestureCanHover", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("reads a finger's gesture as hoverless on a hover-capable device", () => {
    stubMatchMedia(false);
    expect(gestureCanHover("touch")).toBe(false);
  });

  it("reads a cursor's gesture as hovering on a coarse-pointer device", () => {
    stubMatchMedia(true);
    expect(gestureCanHover("mouse")).toBe(true);
  });

  // A stylus reports its position before it touches down.
  it("reads a pen's gesture as hovering", () => {
    stubMatchMedia(true);
    expect(gestureCanHover("pen")).toBe(true);
  });

  // A keyboard activation, a synthesized event, or a browser without pointer
  // events names no pointer.
  it("falls back to the primary pointer where the gesture names none", () => {
    stubMatchMedia(true);
    expect(gestureCanHover(null)).toBe(false);
    expect(gestureCanHover(undefined)).toBe(false);
    expect(gestureCanHover("")).toBe(false);

    stubMatchMedia(false);
    expect(gestureCanHover(null)).toBe(true);
  });

  it("falls back to the primary pointer for a pointer kind it does not know", () => {
    stubMatchMedia(false);
    expect(gestureCanHover("eye-tracker")).toBe(true);
    stubMatchMedia(true);
    expect(gestureCanHover("eye-tracker")).toBe(false);
  });
});
