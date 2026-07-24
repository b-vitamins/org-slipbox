import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createRoot } from "solid-js";
import { describe, expect, it } from "vitest";

import { MIN_TERM_CHARACTERS, createSearchController } from "./search.js";
import type { QueryUrl } from "./query-url.js";
import type { Scheduler } from "../data/scheduler.js";

/** A scheduler whose callbacks fire only when `flush` is called. */
function manualScheduler(): Scheduler & { flush: () => void; pending: () => number } {
  const jobs = new Map<number, () => void>();
  let nextHandle = 1;
  return {
    set: (fn) => {
      const handle = nextHandle++;
      jobs.set(handle, fn);
      return handle;
    },
    clear: (handle) => {
      jobs.delete(handle);
    },
    flush: () => {
      for (const fn of [...jobs.values()]) {
        fn();
      }
      jobs.clear();
    },
    pending: () => jobs.size,
  };
}

function memoryQueryUrl(initial: string | null = null): QueryUrl & {
  readonly writes: (string | null)[];
} {
  let current = initial;
  const writes: (string | null)[] = [];
  return {
    read: () => current,
    replace: (term) => {
      current = term;
      writes.push(term);
    },
    writes,
  };
}

describe("createSearchController", () => {
  it("shows the query immediately but settles the term after the debounce", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const search = createSearchController(180, scheduler);

      search.input("kmea");
      expect(search.query()).toBe("kmea");
      expect(search.term()).toBeNull();
      expect(scheduler.pending()).toBe(1);

      scheduler.flush();
      expect(search.term()).toBe("kmea");
      dispose();
    });
  });

  it("keeps only the last keystroke's term when typing a burst", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const search = createSearchController(180, scheduler);

      search.input("k");
      search.input("km");
      search.input("kmeans");
      expect(scheduler.pending()).toBe(1);

      scheduler.flush();
      expect(search.term()).toBe("kmeans");
      dispose();
    });
  });

  it("trims surrounding whitespace out of the settled term", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const search = createSearchController(180, scheduler);

      search.input("  gibbs  ");
      scheduler.flush();
      expect(search.query()).toBe("  gibbs  ");
      expect(search.term()).toBe("gibbs");
      dispose();
    });
  });

  it("drops the term to null the moment the field is emptied, without debounce", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const search = createSearchController(180, scheduler);

      search.input("kmeans");
      scheduler.flush();
      expect(search.term()).toBe("kmeans");

      search.input("");
      expect(search.term()).toBeNull();
      expect(scheduler.pending()).toBe(0);
      dispose();
    });
  });

  it("treats an all-whitespace field as empty, never firing a blank search", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const search = createSearchController(180, scheduler);

      search.input("   ");
      expect(search.term()).toBeNull();
      expect(scheduler.pending()).toBe(0);
      dispose();
    });
  });

  it("settles the term immediately when the debounce window is non-positive", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const search = createSearchController(0, scheduler);

      search.input("kmeans");
      expect(search.term()).toBe("kmeans");
      expect(scheduler.pending()).toBe(0);
      dispose();
    });
  });

  it("clears the field and term together and cancels a pending settle", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const search = createSearchController(180, scheduler);

      search.input("kmeans");
      expect(scheduler.pending()).toBe(1);

      search.clear();
      expect(search.query()).toBe("");
      expect(search.term()).toBeNull();
      expect(scheduler.pending()).toBe(0);
      dispose();
    });
  });

  it("reads as pending exactly while the field shows an unanswered query", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const search = createSearchController(180, scheduler);

      expect(search.pending()).toBe(false);
      search.input("kmeans");
      expect(search.pending()).toBe(true);
      scheduler.flush();
      expect(search.pending()).toBe(false);

      // Whitespace-only differences are not a new query: the term is trimmed.
      search.input("kmeans  ");
      expect(search.pending()).toBe(false);
      search.input("");
      expect(search.pending()).toBe(false);
      dispose();
    });
  });

  it("settles the field's own text on demand, cancelling the pending window", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const url = memoryQueryUrl();
      const search = createSearchController(180, scheduler, url);

      search.input("kmeans");
      search.settle();
      expect(search.term()).toBe("kmeans");
      expect(search.pending()).toBe(false);
      expect(scheduler.pending()).toBe(0);
      expect(url.writes).toEqual(["kmeans"]);

      search.input("   ");
      search.settle();
      expect(search.term()).toBeNull();
      dispose();
    });
  });

  it("holds a query with no searchable word as no term at all", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const url = memoryQueryUrl();
      const search = createSearchController(180, scheduler, url);

      // The server rejects a one-character query.
      search.input("k");
      expect(search.term()).toBeNull();
      expect(search.awaitingWord()).toBe(true);
      expect(scheduler.pending()).toBe(0);
      expect(search.pending()).toBe(false);
      expect(url.writes).toEqual([]);

      search.input("km");
      expect(search.awaitingWord()).toBe(false);
      scheduler.flush();
      expect(search.term()).toBe("km");
      dispose();
    });
  });

  it("measures a searchable word in characters, the way the server does", () => {
    createRoot((dispose) => {
      const search = createSearchController(0, manualScheduler());

      search.input("KL");
      expect(search.term()).toBe("KL");
      search.input("(ab)");
      expect(search.term()).toBe("(ab)");
      search.input("(a)");
      expect(search.term()).toBeNull();
      search.input("--");
      expect(search.term()).toBeNull();
      search.input("a of the");
      expect(search.term()).toBe("a of the");
      // Characters, not bytes: a CJK character is three UTF-8 bytes and one
      // character.
      search.input("猫");
      expect(search.term()).toBeNull();
      search.input("東京");
      expect(search.term()).toBe("東京");
      // Characters, not UTF-16 units either: an astral character is a surrogate
      // pair, and counting the pair would search on half a letter.
      search.input("𝔥");
      expect(search.term()).toBeNull();
      search.input("𝔥𝔨");
      expect(search.term()).toBe("𝔥𝔨");
      dispose();
    });
  });

  it("seeds the query and term from a restored ?q= URL", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const search = createSearchController(0, scheduler, memoryQueryUrl("gibbs"));

      expect(search.query()).toBe("gibbs");
      expect(search.term()).toBe("gibbs");
      dispose();
    });
  });

  it("restores a too-short ?q= as a field to finish, not a search to run", () => {
    createRoot((dispose) => {
      const search = createSearchController(0, manualScheduler(), memoryQueryUrl("g"));

      expect(search.query()).toBe("g");
      expect(search.term()).toBeNull();
      expect(search.awaitingWord()).toBe(true);
      dispose();
    });
  });

  it("mirrors only the settled term to the URL, never a mid-debounce keystroke", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const url = memoryQueryUrl();
      const search = createSearchController(180, scheduler, url);

      search.input("k");
      search.input("km");
      search.input("kmeans");
      expect(url.writes).toEqual([]);

      scheduler.flush();
      expect(url.writes).toEqual(["kmeans"]);
      dispose();
    });
  });

  it("mirrors a cleared field as a null term", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const url = memoryQueryUrl();
      const search = createSearchController(0, scheduler, url);

      search.input("kmeans");
      search.clear();
      expect(url.writes).toEqual(["kmeans", null]);
      dispose();
    });
  });

  it("holds the floor the index holds, in the unit the index counts", () => {
    // A browser cannot import a Rust constant, so this controller restates the
    // floor `slipbox-core` declares and this test reads that declaration back.
    const core = readFileSync(
      resolve(process.cwd(), "../../slipbox-core/src/nodes.rs"),
      "utf8",
    );
    const declaration = /pub const MIN_SEARCH_TERM_CHARACTERS: usize = (\d+);/.exec(
      core,
    );
    expect(declaration?.[1]).toBe(String(MIN_TERM_CHARACTERS));
  });
});
