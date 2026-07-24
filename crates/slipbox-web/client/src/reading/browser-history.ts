/*
 * The `StackHistory` implementation backed by `window.history` and `location`.
 *
 * A caller wires `onPopState` to the stack's `sync` so back/forward re-reads the
 * URL.
 */

import type { StackHistory } from "./stack.js";

export function browserHistory(): StackHistory {
  return {
    read: () => window.location.search,
    push: (url) => {
      const next = url === "" ? window.location.pathname : url;
      window.history.pushState(null, "", next);
    },
  };
}

/** Subscribe `handler` to browser back/forward; returns an unsubscribe. */
export function onPopState(handler: () => void): () => void {
  window.addEventListener("popstate", handler);
  return () => window.removeEventListener("popstate", handler);
}
