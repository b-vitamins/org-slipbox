/*
 * A cancellable timer seam for debounced behavior. The default binds the window
 * timer functions; a test injects a manual one and flushes it.
 */

/** A cancellable timer, defaulting to the window timer functions. */
export interface Scheduler {
  readonly set: (fn: () => void, ms: number) => number;
  readonly clear: (handle: number) => void;
}

export const WINDOW_SCHEDULER: Scheduler = {
  set: (fn, ms) => window.setTimeout(fn, ms),
  clear: (handle) => window.clearTimeout(handle),
};
