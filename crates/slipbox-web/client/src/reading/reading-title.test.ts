import { describe, expect, it } from "vitest";

import { ApiError } from "../api/client.js";
import {
  columnTitle,
  revealedColumn,
  unresolvedTitle,
} from "./reading-title.js";
import { columnStates, type SpineMetrics } from "./spine-geometry.js";

/** Three 625px columns in a frame that holds one of them and a ladder. */
const FRAME: SpineMetrics = {
  columnWidth: 625,
  sliver: 40,
  viewport: 1200,
  scrollWidth: 3 * 625,
};

describe("revealedColumn", () => {
  it("is the first column the ladder has not pinned", () => {
    // At the head of a trail, where nothing has been pulled off its place in the
    // flow: the tab names the note under the reader rather than the last opened.
    expect(revealedColumn(3, 0, FRAME, false)).toBe(0);
  });

  it("is the column the reader stands in, not the one resting ahead of it", () => {
    // 585px along, the states are these: the open column floats over the sliver
    // behind it, and the column at rest is the one past it, which the reader has
    // not reached. So the states cannot name the reading position and the pinning
    // they are computed from is what does.
    expect(columnStates(3, 585, FRAME, false)).toEqual([
      "obscured",
      "overlay",
      "resting",
    ]);
    expect(revealedColumn(3, 585, FRAME, false)).toBe(1);
  });

  it("is the frontmost column at the end of the trail", () => {
    // Scrolled to the end: every column behind the last is pinned under the
    // ladder, so the only column left to name is the one just opened.
    expect(revealedColumn(3, 675, FRAME, false)).toBe(2);
  });

  it("is the frontmost column when the whole spine stands in the frame", () => {
    // Nothing is pinned or cut, so no column is the reading position; that also
    // describes the narrow layout, which rests every column it stacks.
    expect(revealedColumn(2, 0, { ...FRAME, scrollWidth: 2 * 625 }, true)).toBe(1);
  });

  it("is undefined for an empty stack, so the tab falls back to the product name", () => {
    expect(revealedColumn(0, 0, FRAME, false)).toBeUndefined();
    expect(revealedColumn(0, 0, FRAME, true)).toBeUndefined();
  });

  it("falls back to the frontmost column where every column is pinned", () => {
    // Past the end of the scroller, which no scroll reaches: the fallback is what
    // keeps the tab named rather than blank.
    expect(revealedColumn(3, 4 * 625, FRAME, false)).toBe(2);
  });
});

describe("columnTitle", () => {
  it("names the tab after the note the column read", () => {
    expect(columnTitle({ title: "Gradient descent" })).toBe("Gradient descent");
  });

  it("names the tab after the failure when the column could not read it", () => {
    expect(columnTitle({ error: new ApiError(404, "not-found", "no note") })).toBe(
      "Note not found",
    );
  });

  it("has nothing to say for a column still reading", () => {
    expect(columnTitle(undefined)).toBeUndefined();
  });
});

describe("unresolvedTitle", () => {
  // The tab strip is where a dead share link and an unreachable slipbox are
  // visible at once, so they are named apart there.
  it("names a missing note as missing", () => {
    expect(unresolvedTitle(new ApiError(404, "not-found", "no note"))).toBe(
      "Note not found",
    );
  });

  it("names any other failure as an unavailable surface", () => {
    expect(unresolvedTitle(new ApiError(503, "unavailable", "daemon is down"))).toBe(
      "Unavailable",
    );
    expect(unresolvedTitle(new Error("network"))).toBe("Unavailable");
  });
});
