import { describe, expect, it } from "vitest";

import { createNoteIdentities, type NoteIdentities } from "./note-identity.js";
import {
  createReadingStack,
  decodeStack,
  encodeStack,
  reduceFollow,
  type StackHistory,
} from "./stack.js";

/** An in-memory history so the reducer runs without a browser. */
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

/** Identities that know nothing, so references compare by spelling alone. */
function noIdentities(): NoteIdentities {
  return createNoteIdentities();
}

/** Identities in which `key` and `id:<id>` name one note, as the API reports. */
function identify(key: string, id: string): NoteIdentities {
  const identities = createNoteIdentities();
  identities.learn({ node_key: key, explicit_id: id } as Parameters<
    NoteIdentities["learn"]
  >[0]);
  return identities;
}

describe("stack URL codec", () => {
  it("round-trips a multi-note stack through the query string", () => {
    const keys = ["heading:a.org:1", "heading:b.org:2", "id:xyz"];
    expect(decodeStack(encodeStack(keys))).toEqual(keys);
  });

  it("decodes an empty or note-less URL to an empty stack", () => {
    expect(decodeStack("")).toEqual([]);
    expect(decodeStack("?other=1")).toEqual([]);
    expect(encodeStack([])).toBe("");
  });

  it("collapses a reference an address repeats to its first position", () => {
    expect(decodeStack("?note=root&stacked=a&stacked=root&stacked=b")).toEqual([
      "root",
      "a",
      "b",
    ]);
    expect(decodeStack("?note=root&stacked=a&stacked=a")).toEqual(["root", "a"]);
  });

  it("leaves an address of distinct references in the order it names them", () => {
    expect(decodeStack("?note=root&stacked=a&stacked=b")).toEqual([
      "root",
      "a",
      "b",
    ]);
  });

  it("encodes the root as `note` and the rest as repeated `stacked`", () => {
    const encoded = encodeStack(["root", "a", "b"]);
    const params = new URLSearchParams(encoded.slice(1));
    expect(params.get("note")).toBe("root");
    expect(params.getAll("stacked")).toEqual(["a", "b"]);
  });
});

describe("reduceFollow", () => {
  it("appends the target when following from the last column", () => {
    expect(reduceFollow(["a", "b"], 1, "c", noIdentities())).toEqual({
      keys: ["a", "b", "c"],
      index: 2,
      wasOpen: false,
    });
  });

  it("prunes columns to the right when following from a mid-stack column", () => {
    expect(reduceFollow(["a", "b", "c", "d"], 1, "e", noIdentities())).toEqual({
      keys: ["a", "b", "e"],
      index: 2,
      wasOpen: false,
    });
  });

  it("reveals the existing column when the target is already the next one", () => {
    expect(reduceFollow(["a", "b", "c"], 0, "b", noIdentities())).toEqual({
      keys: ["a", "b", "c"],
      index: 1,
      wasOpen: true,
    });
  });

  // The duplicate a narrower guard let through: following a link back to a note
  // open further left — the root, say — must reveal it, not open a second copy.
  it("reveals a column anywhere in the stack rather than duplicating it", () => {
    expect(reduceFollow(["a", "b", "c"], 1, "a", noIdentities())).toEqual({
      keys: ["a", "b", "c"],
      index: 0,
      wasOpen: true,
    });
  });

  it("recognizes an id reference to a note open under its key", () => {
    const identities = identify("file:nf.org", "1698");
    expect(
      reduceFollow(["file:nf.org", "file:cov.org"], 1, "id:1698", identities),
    ).toEqual({ keys: ["file:nf.org", "file:cov.org"], index: 0, wasOpen: true });
  });

  it("recognizes a key reference to a note open under its id", () => {
    const identities = identify("file:nf.org", "1698");
    expect(
      reduceFollow(["id:1698", "file:cov.org"], 1, "file:nf.org", identities),
    ).toEqual({ keys: ["id:1698", "file:cov.org"], index: 0, wasOpen: true });
  });

  it("opens a distinct note whose reference was never resolved", () => {
    const identities = identify("file:nf.org", "1698");
    expect(reduceFollow(["file:nf.org"], 0, "id:other", identities)).toEqual({
      keys: ["file:nf.org", "id:other"],
      index: 1,
      wasOpen: false,
    });
  });
});

describe("createReadingStack", () => {
  it("initializes from the current URL", () => {
    const stack = createReadingStack(memoryHistory("?note=root&stacked=a"));
    expect(stack.keys()).toEqual(["root", "a"]);
  });

  // The first load pushes no entry of its own, so the collapse has to correct the
  // entry it arrived on: a push here would leave a repeating address behind Back.
  it("opens a repeating first address once and rewrites it in place", () => {
    const pushed: string[] = [];
    const replaced: string[] = [];
    let url = "?note=root&stacked=a&stacked=root";
    const stack = createReadingStack({
      read: () => url,
      push: (next) => {
        pushed.push(next);
        url = next;
      },
      replace: (next) => {
        replaced.push(next);
        url = next;
      },
    });

    expect(stack.keys()).toEqual(["root", "a"]);
    expect(replaced).toEqual([encodeStack(["root", "a"])]);
    expect(pushed).toEqual([]);
  });

  it("writes nothing to the history for an address of distinct references", () => {
    const written: string[] = [];
    const stack = createReadingStack({
      read: () => "?note=root&stacked=a",
      push: (url) => {
        written.push(url);
      },
      replace: (url) => {
        written.push(url);
      },
    });

    expect(stack.keys()).toEqual(["root", "a"]);
    expect(written).toEqual([]);
  });

  it("collapses a repeating address arrived at through history", () => {
    const history = memoryHistory("?note=root");
    const stack = createReadingStack(history);

    history.push("?note=other&stacked=z&stacked=other");
    stack.sync();

    expect(stack.keys()).toEqual(["other", "z"]);
    expect(history.read()).toBe(encodeStack(["other", "z"]));
  });

  it("commits a follow to both the signal and the history", () => {
    const history = memoryHistory("?note=root");
    const stack = createReadingStack(history, noIdentities());
    expect(stack.follow(0, "child")).toBe(1);
    expect(stack.keys()).toEqual(["root", "child"]);
    expect(decodeStack(history.read())).toEqual(["root", "child"]);
  });

  // Revealing an open note is not a navigation: it must leave the URL alone, so
  // Back remains the step out of the reading path.
  it("pushes no history entry when the target is already open", () => {
    const history = memoryHistory("?note=root&stacked=child");
    const stack = createReadingStack(history, noIdentities());
    history.push("?sentinel=1");

    expect(stack.follow(1, "root")).toBe(0);
    expect(stack.keys()).toEqual(["root", "child"]);
    expect(history.read()).toBe("?sentinel=1");
  });

  it("open replaces the whole stack", () => {
    const stack = createReadingStack(memoryHistory("?note=root&stacked=a&stacked=b"));
    stack.open("fresh");
    expect(stack.keys()).toEqual(["fresh"]);
  });

  it("sync re-reads the stack after a history change", () => {
    const history = memoryHistory("?note=root");
    const stack = createReadingStack(history);
    history.push("?note=other&stacked=z");
    stack.sync();
    expect(stack.keys()).toEqual(["other", "z"]);
  });
});
