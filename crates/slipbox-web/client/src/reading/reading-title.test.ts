import { describe, expect, it } from "vitest";

import { ApiError } from "../api/client.js";
import { frontmostReference, unresolvedTitle } from "./reading-title.js";

describe("frontmostReference", () => {
  it("is the single note when the stack holds one", () => {
    expect(frontmostReference(["notes/a.org::0"])).toBe("notes/a.org::0");
  });

  it("is the rightmost note of a trail — the one in focus", () => {
    expect(
      frontmostReference(["notes/a.org::0", "notes/b.org::0", "id:abc"]),
    ).toBe("id:abc");
  });

  it("is undefined for an empty stack, so the tab falls back to the product name", () => {
    expect(frontmostReference([])).toBeUndefined();
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
