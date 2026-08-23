import { describe, expect, it } from "vitest";

import { decodeTerm, encodeTerm } from "./term-url.js";

describe("decodeTerm", () => {
  it("reads the key from a leading-? search string", () => {
    expect(decodeTerm("?term=notes%2Fentropy.org%3A%3A0")).toBe(
      "notes/entropy.org::0",
    );
  });

  it("reads the key from a bare query string", () => {
    expect(decodeTerm("term=notes%2Fentropy.org%3A%3A0")).toBe(
      "notes/entropy.org::0",
    );
  });

  it("is null when the parameter is absent", () => {
    expect(decodeTerm("?view=glossary&q=entropy")).toBeNull();
  });

  it("is null for an empty or whitespace key, which names no term", () => {
    expect(decodeTerm("?term=")).toBeNull();
    expect(decodeTerm("?term=%20%20")).toBeNull();
    expect(decodeTerm("")).toBeNull();
  });

  it("reads the key beside the parameters it shares the query with", () => {
    expect(decodeTerm("?view=glossary&q=entro&term=notes%2Fa.org%3A%3A0")).toBe(
      "notes/a.org::0",
    );
  });
});

describe("encodeTerm", () => {
  it("encodes a key as a ?term= query", () => {
    expect(encodeTerm("notes/a.org::0", "")).toBe("?term=notes%2Fa.org%3A%3A0");
  });

  it("keeps the rest of the query when naming the term", () => {
    // The surface mode and the search term live in the same query and are owned
    // from the other side, so a whole-URL write would drop them: naming a term
    // would take the reader out of the glossary.
    expect(encodeTerm("notes/a.org::0", "?view=glossary&q=entro")).toBe(
      "?view=glossary&q=entro&term=notes%2Fa.org%3A%3A0",
    );
  });

  it("leaves the reading stack's own parameters standing", () => {
    expect(encodeTerm("notes/a.org::0", "/reader?note=notes%2Fb.org&stacked=2")).toBe(
      "/reader?note=notes%2Fb.org&stacked=2&term=notes%2Fa.org%3A%3A0",
    );
  });

  it("replaces an existing key rather than repeating the parameter", () => {
    expect(encodeTerm("notes/b.org::0", "?term=notes%2Fa.org%3A%3A0")).toBe(
      "?term=notes%2Fb.org%3A%3A0",
    );
  });

  it("removes the key while leaving the rest of the query standing", () => {
    expect(encodeTerm(null, "?view=glossary&term=notes%2Fa.org%3A%3A0")).toBe(
      "?view=glossary",
    );
    expect(encodeTerm("  ", "?view=glossary&term=notes%2Fa.org%3A%3A0")).toBe(
      "?view=glossary",
    );
  });

  it("drops a query left with nothing in it, keeping the path", () => {
    expect(encodeTerm(null, "/reader?term=notes%2Fa.org%3A%3A0")).toBe("/reader");
  });

  it("round-trips a key through encode then decode", () => {
    expect(decodeTerm(encodeTerm("notes/deep/x.org::12", ""))).toBe(
      "notes/deep/x.org::12",
    );
  });
});
