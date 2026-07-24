import { describe, expect, it } from "vitest";

import { decodeCursor, encodeCursor } from "./cursor-history.js";

describe("decodeCursor", () => {
  it("reads the cursor a history entry carries", () => {
    expect(decodeCursor({ entryCursor: "file:notes/gibbs.org" })).toBe(
      "file:notes/gibbs.org",
    );
  });

  it("is null for an entry that holds no state", () => {
    expect(decodeCursor(null)).toBeNull();
    expect(decodeCursor(undefined)).toBeNull();
  });

  it("is null for state that names no cursor", () => {
    // An entry pushed by the reading stack, which records no cursor of its own.
    expect(decodeCursor({ somethingElse: 1 })).toBeNull();
  });

  it("is null for a cursor of the wrong shape", () => {
    // State from another build, or from another page sharing the tab's history.
    expect(decodeCursor({ entryCursor: 17 })).toBeNull();
    expect(decodeCursor({ entryCursor: null })).toBeNull();
    expect(decodeCursor({ entryCursor: "" })).toBeNull();
  });

  it("is null for a primitive state value", () => {
    expect(decodeCursor("file:notes/gibbs.org")).toBeNull();
  });
});

describe("encodeCursor", () => {
  it("records a cursor on an entry that held no state", () => {
    expect(encodeCursor(null, "file:notes/gibbs.org")).toEqual({
      entryCursor: "file:notes/gibbs.org",
    });
  });

  it("keeps the state it does not own", () => {
    expect(encodeCursor({ scroll: 12 }, "file:notes/gibbs.org")).toEqual({
      scroll: 12,
      entryCursor: "file:notes/gibbs.org",
    });
  });

  it("replaces a cursor already recorded", () => {
    expect(encodeCursor({ entryCursor: "file:notes/a.org" }, "file:notes/b.org")).toEqual(
      { entryCursor: "file:notes/b.org" },
    );
  });

  it("removes the cursor for a null key, leaving the rest standing", () => {
    expect(encodeCursor({ scroll: 12, entryCursor: "file:notes/a.org" }, null)).toEqual(
      { scroll: 12 },
    );
  });

  it("is null when clearing leaves nothing on the entry", () => {
    expect(encodeCursor({ entryCursor: "file:notes/a.org" }, null)).toBeNull();
    expect(encodeCursor(null, null)).toBeNull();
  });

  it("does not write through to the state it was handed", () => {
    // The surface hands over the live `history.state`, which must not be mutated
    // in place: the entry changes only when the browser is told to change it.
    const held = { entryCursor: "file:notes/a.org" };
    encodeCursor(held, "file:notes/b.org");
    expect(held).toEqual({ entryCursor: "file:notes/a.org" });
  });

  it("round-trips a cursor through encode then decode", () => {
    expect(decodeCursor(encodeCursor(null, "file:notes/gibbs.org"))).toBe(
      "file:notes/gibbs.org",
    );
  });
});
