import { describe, expect, it } from "vitest";

import { decodeQuery, encodeQuery, replaceParam } from "./query-url.js";

describe("decodeQuery", () => {
  it("reads the term from a leading-? search string", () => {
    expect(decodeQuery("?q=maximum%20principle")).toBe("maximum principle");
  });

  it("reads the term from a bare query string", () => {
    expect(decodeQuery("q=gibbs")).toBe("gibbs");
  });

  it("is null when the parameter is absent", () => {
    expect(decodeQuery("?note=notes%2Fx.org")).toBeNull();
  });

  it("is null for an empty term, so an empty ?q= never searches", () => {
    expect(decodeQuery("?q=")).toBeNull();
  });

  it("is null for an empty search string", () => {
    expect(decodeQuery("")).toBeNull();
  });

  it("is null for an all-whitespace term, which names no search", () => {
    // The surface searches for the trimmed term, and the server rejects an empty
    // `q`. Seeding from `?q=%20` would render a search state with a blank field.
    expect(decodeQuery("?q=%20%20")).toBeNull();
    expect(decodeQuery("?q=+")).toBeNull();
  });

  it("trims a padded term to the term the surface searches for", () => {
    expect(decodeQuery("?q=%20gibbs%20")).toBe("gibbs");
  });
});

describe("encodeQuery", () => {
  it("encodes a term as a ?q= query", () => {
    expect(encodeQuery("gibbs", "/")).toBe("/?q=gibbs");
  });

  it("encodes a term against an empty URL as a query-only address", () => {
    // The reading column's search link is query-only so it resolves against
    // whatever path the surface is served under.
    expect(encodeQuery("gibbs", "")).toBe("?q=gibbs");
  });

  it("percent-encodes spaces and reserved characters", () => {
    expect(encodeQuery("maximum principle", "")).toBe("?q=maximum+principle");
  });

  it("falls back to the bare path for a null term", () => {
    expect(encodeQuery(null, "/reader")).toBe("/reader");
  });

  it("falls back to the bare path for an all-whitespace term", () => {
    expect(encodeQuery("   ", "/reader")).toBe("/reader");
  });

  it("keeps the rest of the query when setting the term", () => {
    // The surface mode lives in the same query under `?view=`. A term written as
    // a URL of its own would drop it, so typing in the glossary's search box
    // would move the reader back to the note entry.
    expect(encodeQuery("gibbs", "?view=glossary")).toBe("?view=glossary&q=gibbs");
    expect(encodeQuery("gibbs", "/reader?view=review")).toBe(
      "/reader?view=review&q=gibbs",
    );
  });

  it("removes the term while leaving the rest of the query standing", () => {
    expect(encodeQuery(null, "?q=gibbs&view=glossary")).toBe("?view=glossary");
    expect(encodeQuery("  ", "?q=gibbs&view=review")).toBe("?view=review");
  });

  it("replaces an existing term rather than repeating the parameter", () => {
    expect(encodeQuery("prior", "?q=gibbs")).toBe("?q=prior");
  });

  it("round-trips a term through encode then decode", () => {
    expect(decodeQuery(encodeQuery("k-means objective", ""))).toBe(
      "k-means objective",
    );
  });
});

describe("replaceParam", () => {
  it("adds a parameter to a URL that carries none", () => {
    expect(replaceParam("/reader", "view", "review")).toBe("/reader?view=review");
  });

  it("leaves the parameters it does not name alone", () => {
    expect(replaceParam("?q=gibbs&view=glossary", "view", "review")).toBe(
      "?q=gibbs&view=review",
    );
  });

  it("removes a parameter for a null value", () => {
    expect(replaceParam("?q=gibbs&view=glossary", "view", null)).toBe("?q=gibbs");
  });

  it("drops a query left with nothing in it, keeping the path", () => {
    expect(replaceParam("/reader?view=glossary", "view", null)).toBe("/reader");
    expect(replaceParam("?view=glossary", "view", null)).toBe("");
  });
});
