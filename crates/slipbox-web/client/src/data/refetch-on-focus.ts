/*
 * Refetch-on-focus: re-run registered resource fetchers when the tab returns to
 * the foreground.
 *
 * The registry is a module singleton, so one pair of listeners serves every
 * resource.
 */

type RefetchFn = () => void;

const registry = new Set<RefetchFn>();
let installed = false;
let lastRefetchAt = 0;

/**
 * Minimum gap between focus-driven refetch sweeps. `focus` and
 * `visibilitychange` both fire when a tab is re-selected, so the pair is
 * collapsed into one sweep; separate refocus events still land past the window.
 */
const REFETCH_GUARD_MS = 250;

/** `performance.now`, falling back to `Date.now` where it is unavailable. */
function now(): number {
  return typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
}

function isForeground(): boolean {
  // A non-DOM context counts as foregrounded, so a refetch is never suppressed
  // there.
  if (typeof document === "undefined") {
    return true;
  }
  return document.visibilityState !== "hidden";
}

function sweep(): void {
  if (!isForeground()) {
    return;
  }
  const at = now();
  if (at - lastRefetchAt < REFETCH_GUARD_MS) {
    return;
  }
  lastRefetchAt = at;
  for (const refetch of registry) {
    refetch();
  }
}

function install(): void {
  if (installed || typeof window === "undefined") {
    return;
  }
  installed = true;
  window.addEventListener("visibilitychange", sweep);
  window.addEventListener("focus", sweep);
}

/**
 * Register a fetcher to run on the next foreground transition. Returns an
 * unregister function; call it on cleanup.
 */
export function onRefocus(refetch: RefetchFn): () => void {
  install();
  registry.add(refetch);
  return () => {
    registry.delete(refetch);
  };
}

/** Test-only reset of the module singleton's registry and guard timestamp. */
export function __resetRefocusForTests(): void {
  registry.clear();
  lastRefetchAt = 0;
}
