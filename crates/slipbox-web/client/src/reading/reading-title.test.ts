import { describe, expect, it } from "vitest";

import { ApiError } from "../api/client.js";
import {
  columnTitle,
  revealedColumn,
  unresolvedTitle,
} from "./reading-title.js";
import { columnStates, type SpineMetrics } from "./spine-geometry.js";

const FRAME: SpineMetrics = {
  columnWidth: 625,
  sliver: 40,
  viewport: 1200,
  scrollWidth: 3 * 625,
};

describe("revealedColumn", () => {
  it("is the first column the ladder has not pinned", () => {
    expect(revealedColumn(3, 0, FRAME, false)).toBe(0);
  });

  it("is the column the reader stands in, not the one resting ahead of it", () => {
    expect(columnStates(3, 585, FRAME, false)).toEqual([
      "obscured",
      "overlay",
      "resting",
    ]);
    expect(revealedColumn(3, 585, FRAME, false)).toBe(1);
  });

  it("is the frontmost column at the end of the trail", () => {
    expect(revealedColumn(3, 675, FRAME, false)).toBe(2);
  });

  it("is the frontmost column when the whole spine stands in the frame", () => {
    expect(revealedColumn(2, 0, { ...FRAME, scrollWidth: 2 * 625 }, true)).toBe(1);
  });

  it("is undefined for an empty stack, so the tab falls back to the product name", () => {
    expect(revealedColumn(0, 0, FRAME, false)).toBeUndefined();
    expect(revealedColumn(0, 0, FRAME, true)).toBeUndefined();
  });

  it("falls back to the frontmost column where every column is pinned", () => {
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
