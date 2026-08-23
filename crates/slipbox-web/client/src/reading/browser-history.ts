import type { StackHistory } from "./stack.js";

function destination(url: string): string {
  return url === "" ? window.location.pathname : url;
}

const ENTRY_ADDRESS_KEY = "entryAddress";

export function readEntryAddress(): string {
  const state: unknown = window.history.state;
  if (typeof state !== "object" || state === null) {
    return "";
  }
  const held = (state as Record<string, unknown>)[ENTRY_ADDRESS_KEY];
  return typeof held === "string" ? held : "";
}

/** Preserve other owners' history state while updating the entry address. */
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
    push: (url) => {
      window.history.pushState(window.history.state, "", destination(url));
    },
    replace: (url) => {
      window.history.replaceState(window.history.state, "", destination(url));
    },
  };
}

export function onPopState(handler: () => void): () => void {
  window.addEventListener("popstate", handler);
  return () => window.removeEventListener("popstate", handler);
}
