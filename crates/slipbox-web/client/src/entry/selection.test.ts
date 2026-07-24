import { describe, expect, it } from "vitest";

import { markedIndex, moveSelection } from "./selection.js";

describe("moveSelection", () => {
  it("lands on the first option when arrowing down from nothing", () => {
    expect(moveSelection(null, "next", 5)).toBe(0);
  });

  it("lands on the last option when arrowing up from nothing", () => {
    expect(moveSelection(null, "previous", 5)).toBe(4);
  });

  it("advances and retreats through the list", () => {
    expect(moveSelection(1, "next", 5)).toBe(2);
    expect(moveSelection(3, "previous", 5)).toBe(2);
  });

  it("wraps past the end back to the start, and before the start to the end", () => {
    expect(moveSelection(4, "next", 5)).toBe(0);
    expect(moveSelection(0, "previous", 5)).toBe(4);
  });

  it("has nothing to highlight in an empty list", () => {
    expect(moveSelection(null, "next", 0)).toBeNull();
    expect(moveSelection(0, "previous", 0)).toBeNull();
  });
});

describe("markedIndex", () => {
  const key = (option: { id: string }): string => option.id;

  it("has nothing to resolve before anything is highlighted", () => {
    expect(markedIndex(null, [{ id: "a" }], key)).toBeNull();
  });

  it("follows the marked option to wherever the list now holds it", () => {
    // A list re-read answers the same search, so the option the reader arrowed
    // to is still there — at whatever row the new answer puts it.
    expect(markedIndex("c", [{ id: "a" }, { id: "b" }, { id: "c" }], key)).toBe(2);
    expect(markedIndex("c", [{ id: "c" }, { id: "a" }], key)).toBe(0);
  });

  it("drops the highlight once the list no longer holds what it marked", () => {
    expect(markedIndex("c", [{ id: "a" }, { id: "b" }], key)).toBeNull();
    expect(markedIndex("c", [], key)).toBeNull();
  });
});
