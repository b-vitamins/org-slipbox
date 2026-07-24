/*
 * The glance intent controller.
 *
 * Hovering a link should not fire a fetch on every quick pass-over, so showing
 * a preview is delayed by a short intent window; dismissing is instant. Once a
 * preview is already visible, moving to another link swaps it immediately —
 * the reader has clearly committed to glancing, so re-imposing the delay would
 * feel laggy. The timer is injectable so the debounce is unit-testable without
 * real time.
 */

import { createSignal, type Accessor } from "solid-js";

import type { GlanceRequest } from "../org/navigation.jsx";

/** A cancellable timer seam, defaulting to the window timer functions. */
export interface Scheduler {
  readonly set: (fn: () => void, ms: number) => number;
  readonly clear: (handle: number) => void;
}

const WINDOW_SCHEDULER: Scheduler = {
  set: (fn, ms) => window.setTimeout(fn, ms),
  clear: (handle) => window.clearTimeout(handle),
};

export interface GlanceController {
  readonly request: Accessor<GlanceRequest | null>;
  /** Request a preview (after the intent delay) or dismiss one (`null`, now). */
  readonly glance: (next: GlanceRequest | null) => void;
  /** Cancel any pending show and clear the current preview (for cleanup). */
  readonly cancel: () => void;
}

/** `intent` is the delay in ms before a hover-raised preview shows. */
export function createGlanceController(
  intent = 120,
  scheduler: Scheduler = WINDOW_SCHEDULER,
): GlanceController {
  const [request, setRequest] = createSignal<GlanceRequest | null>(null);
  let pending: number | null = null;

  const clearPending = (): void => {
    if (pending !== null) {
      scheduler.clear(pending);
      pending = null;
    }
  };

  const glance = (next: GlanceRequest | null): void => {
    clearPending();
    if (next === null) {
      setRequest(null);
      return;
    }
    // A preview is already open: swap immediately. Otherwise wait for intent.
    if (request() !== null) {
      setRequest(next);
      return;
    }
    pending = scheduler.set(() => {
      pending = null;
      setRequest(next);
    }, intent);
  };

  const cancel = (): void => {
    clearPending();
    setRequest(null);
  };

  return { request, glance, cancel };
}
