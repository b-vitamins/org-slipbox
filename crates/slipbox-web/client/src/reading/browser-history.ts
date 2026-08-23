/*
 * The `StackHistory` implementation backed by `window.history` and `location`.
 *
 * A caller wires `onPopState` to the stack's `sync` so back/forward re-reads the
 * URL.
 */

import type { StackHistory } from "./stack.js";

/** An empty query addresses the bare path, not a trailing `?`. */
function destination(url: string): string {
  return url === "" ? window.location.pathname : url;
}

/** The key under which the way back sits inside a history entry's state object. */
const ENTRY_ADDRESS_KEY = "entryAddress";

/**
 * The entry-surface address a history entry carries, or `""` when it carries
 * none. Any state that is not an object holding a string under that key answers
 * `""`: entries from another page, or from another build of the client, arrive
 * here too.
 */
export function readEntryAddress(): string {
  const state: unknown = window.history.state;
  if (typeof state !== "object" || state === null) {
    return "";
  }
  const held = (state as Record<string, unknown>)[ENTRY_ADDRESS_KEY];
  return typeof held === "string" ? held : "";
}

/**
 * Record `address` on the current history entry, spliced into the state already
 * there so the fields other owners hold survive (`cursor-history.ts`). An empty
 * address is deleted, and a state left with no keys is written as `null`.
 *
 * It belongs in the state because the address a note is written to cannot hold it
 * (`encodeStack` builds the query from the stack alone), and memory of it dies
 * with a reload while the state travels with the entry.
 */
export function writeEntryAddress(address: string): void {
  const state: unknown = window.history.state;
  const held =
    typeof state === "object" && state !== null
      ? { ...(state as Record<string, unknown>) }
      : {};
  if (address === "") {
    delete held[ENTRY_ADDRESS_KEY];
  } else {
    held[ENTRY_ADDRESS_KEY] = address;
  }
  window.history.replaceState(
    Object.keys(held).length === 0 ? null : held,
    "",
    `${window.location.pathname}${window.location.search}`,
  );
}

export function browserHistory(): StackHistory {
  return {
    read: () => window.location.search,
    // The state rides onto the new entry: the search cursor (`cursor-history.ts`)
    // is recorded on the entry the reader leaves, and the way back to the entry
    // surface is a push too, so a discarded state would land there with none.
    push: (url) => {
      window.history.pushState(window.history.state, "", destination(url));
    },
    // The entry's own state goes back in: `replaceState` rewrites an entry whole,
    // and the search cursor (`cursor-history.ts`) rides on it.
    replace: (url) => {
      window.history.replaceState(window.history.state, "", destination(url));
    },
  };
}

/** Subscribe `handler` to browser back/forward; returns an unsubscribe. */
export function onPopState(handler: () => void): () => void {
  window.addEventListener("popstate", handler);
  return () => window.removeEventListener("popstate", handler);
}
