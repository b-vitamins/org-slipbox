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

export function browserHistory(): StackHistory {
  return {
    read: () => window.location.search,
    push: (url) => {
      window.history.pushState(null, "", destination(url));
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
