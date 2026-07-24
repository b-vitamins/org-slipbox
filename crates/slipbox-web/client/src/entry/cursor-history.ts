/*
 * The entry surface's result cursor, the marked note's slipbox key, held in
 * `history.state` rather than the query: state there travels with Back, Forward, and
 * a reload of the same entry, while a fresh visit to the same address begins with
 * none of it. Written once, as the surface is left: browsers rate-limit the writes.
 */

/** The persistence seam: read this history entry's cursor and write it back. */
export interface CursorHistory {
  /** The cursor recorded on the current entry, or `null` when it holds none. */
  readonly read: () => string | null;
  /** Record `key` on the current entry; `null` clears it. */
  readonly write: (key: string | null) => void;
}

/** The key under which the cursor sits inside a history entry's state object. */
const CURSOR_KEY = "entryCursor";

/**
 * Read a cursor out of a history entry's state. Any state that is not an object
 * carrying a non-empty string under the cursor's key answers `null`: entries from
 * another page, or from another build of the client, arrive here too.
 */
export function decodeCursor(state: unknown): string | null {
  if (typeof state !== "object" || state === null) {
    return null;
  }
  const held = (state as Record<string, unknown>)[CURSOR_KEY];
  return typeof held === "string" && held !== "" ? held : null;
}

/**
 * The state object to record `key` on, spliced into the state already there so
 * fields other owners hold survive. A cleared cursor is deleted, and a state left
 * with no keys encodes as `null`.
 */
export function encodeCursor(state: unknown, key: string | null): unknown {
  const held =
    typeof state === "object" && state !== null
      ? { ...(state as Record<string, unknown>) }
      : {};
  if (key === null) {
    delete held[CURSOR_KEY];
  } else {
    held[CURSOR_KEY] = key;
  }
  return Object.keys(held).length === 0 ? null : held;
}

/** A `CursorHistory` backed by `window.history` and `replaceState`. */
export function browserCursorHistory(): CursorHistory {
  return {
    read: () => decodeCursor(window.history.state),
    // Replace, never push: the cursor annotates the entry the surface occupies.
    // The address goes back in unchanged because `replaceState` rewrites the entry
    // whole, and omitting the URL is not equivalent to leaving it alone in every
    // browser; `?q=` and `?view=` live in it.
    write: (key) => {
      const url = `${window.location.pathname}${window.location.search}`;
      window.history.replaceState(
        encodeCursor(window.history.state, key),
        "",
        url,
      );
    },
  };
}
