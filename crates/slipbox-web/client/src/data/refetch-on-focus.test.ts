import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { __resetRefocusForTests, onRefocus } from "./refetch-on-focus.js";

/** Dispatch the browser focus signal that drives a refetch sweep. */
function fireFocus(): void {
  window.dispatchEvent(new Event("focus"));
}

function setVisibility(state: DocumentVisibilityState): void {
  Object.defineProperty(document, "visibilityState", {
    configurable: true,
    get: () => state,
  });
}

describe("onRefocus", () => {
  beforeEach(() => {
    __resetRefocusForTests();
    setVisibility("visible");
    // Drive the guard interval off a controllable clock.
    let clock = 10_000;
    vi.spyOn(performance, "now").mockImplementation(() => clock);
    vi.stubGlobal("__advanceClock", (ms: number) => {
      clock += ms;
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  function advance(ms: number): void {
    (globalThis as unknown as { __advanceClock: (ms: number) => void }).__advanceClock(ms);
  }

  it("refetches a registered fetcher when the tab is refocused", () => {
    const refetch = vi.fn();
    onRefocus(refetch);

    fireFocus();

    expect(refetch).toHaveBeenCalledTimes(1);
  });

  it("collapses a focus/visibility pair into one sweep within the guard window", () => {
    const refetch = vi.fn();
    onRefocus(refetch);

    // Both events fire on a single tab switch; only the first should run.
    fireFocus();
    window.dispatchEvent(new Event("visibilitychange"));

    expect(refetch).toHaveBeenCalledTimes(1);
  });

  it("refetches again once the guard window elapses", () => {
    const refetch = vi.fn();
    onRefocus(refetch);

    fireFocus();
    advance(300);
    fireFocus();

    expect(refetch).toHaveBeenCalledTimes(2);
  });

  it("does not refetch while the tab is hidden", () => {
    const refetch = vi.fn();
    onRefocus(refetch);

    setVisibility("hidden");
    fireFocus();

    expect(refetch).not.toHaveBeenCalled();
  });

  it("stops refetching after the returned unregister is called", () => {
    const refetch = vi.fn();
    const unregister = onRefocus(refetch);

    unregister();
    fireFocus();

    expect(refetch).not.toHaveBeenCalled();
  });

  it("sweeps every registered fetcher on one refocus", () => {
    const first = vi.fn();
    const second = vi.fn();
    onRefocus(first);
    onRefocus(second);

    fireFocus();

    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);
  });
});
