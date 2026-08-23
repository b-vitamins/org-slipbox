import { describe, expect, it } from "vitest";

import {
  createSurfaceView,
  decodeView,
  encodeView,
} from "./surface-view.js";
import type { StackHistory } from "../reading/stack.js";

/** An in-memory history so the store runs without a browser. */
function memoryHistory(initial = ""): StackHistory {
  let url = initial;
  return {
    read: () => url,
    push: (next) => {
      url = next;
    },
    replace: (next) => {
      url = next;
    },
  };
}

describe("surface-view URL codec", () => {
  it("decodes an explicit glossary view", () => {
    expect(decodeView("?view=glossary")).toBe("glossary");
  });

  it("decodes the glossary's review list as its own view", () => {
    expect(decodeView("?view=review")).toBe("review");
  });

  it("decodes anything else as the default search view", () => {
    expect(decodeView("")).toBe("search");
    expect(decodeView("?view=search")).toBe("search");
    expect(decodeView("?note=abc")).toBe("search");
    expect(decodeView("?view=nonsense")).toBe("search");
  });

  it("encodes each named view explicitly and search as an empty query", () => {
    expect(encodeView("glossary")).toBe("?view=glossary");
    expect(encodeView("review")).toBe("?view=review");
    expect(encodeView("search")).toBe("");
  });

  it("keeps the rest of the query when setting the mode", () => {
    expect(encodeView("glossary", "?q=gibbs")).toBe("?q=gibbs&view=glossary");
    expect(encodeView("review", "?q=gibbs")).toBe("?q=gibbs&view=review");
    expect(encodeView("search", "?q=gibbs&view=glossary")).toBe("?q=gibbs");
  });

  it("drops a query left with nothing in it, keeping the path", () => {
    expect(encodeView("search", "?view=glossary")).toBe("");
    expect(encodeView("search", "/reader?view=glossary")).toBe("/reader");
  });

  it("replaces an existing view rather than repeating the parameter", () => {
    expect(encodeView("glossary", "?view=glossary")).toBe("?view=glossary");
    expect(encodeView("review", "?view=glossary")).toBe("?view=review");
  });
});

describe("createSurfaceView", () => {
  it("starts from the mode named in the initial URL", () => {
    expect(createSurfaceView(memoryHistory("?view=glossary")).mode()).toBe(
      "glossary",
    );
    expect(createSurfaceView(memoryHistory("?view=review")).mode()).toBe("review");
    expect(createSurfaceView(memoryHistory("")).mode()).toBe("search");
  });

  it("shows a mode and mirrors it to history", () => {
    const history = memoryHistory("");
    const pushed: string[] = [];
    const view = createSurfaceView({
      read: history.read,
      push: (url) => {
        pushed.push(url);
        history.push(url);
      },
      replace: history.replace,
    });

    view.show("glossary");
    expect(view.mode()).toBe("glossary");
    expect(pushed).toEqual(["?view=glossary"]);

    view.show("search");
    expect(view.mode()).toBe("search");
    expect(pushed).toEqual(["?view=glossary", ""]);
  });

  it("walks between the glossary's two lists as URL state", () => {
    const history = memoryHistory("");
    const view = createSurfaceView(history);

    view.show("glossary");
    view.show("review");
    expect(view.mode()).toBe("review");
    expect(history.read()).toBe("?view=review");

    view.show("glossary");
    expect(history.read()).toBe("?view=glossary");
  });

  it("preserves a live search term across a mode round trip", () => {
    const history = memoryHistory("?q=gibbs");
    const view = createSurfaceView(history);

    view.show("glossary");
    expect(history.read()).toBe("?q=gibbs&view=glossary");

    view.show("search");
    expect(history.read()).toBe("?q=gibbs");
  });

  it("keeps a live search term across a switch between the glossary lists", () => {
    const history = memoryHistory("?q=entropy&view=glossary");
    const view = createSurfaceView(history);

    view.show("review");
    expect(history.read()).toBe("?q=entropy&view=review");

    view.show("glossary");
    expect(history.read()).toBe("?q=entropy&view=glossary");
  });

  it("re-reads the mode from the URL on sync", () => {
    let url = "?view=glossary";
    const view = createSurfaceView({
      read: () => url,
      push: () => {},
      replace: () => {},
    });
    expect(view.mode()).toBe("glossary");

    url = "?view=review";
    view.sync();
    expect(view.mode()).toBe("review");

    url = "";
    view.sync();
    expect(view.mode()).toBe("search");
  });
});
