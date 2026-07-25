/*
 * The glance intent controller. Showing waits out a short intent window so a
 * pass-over hover fires no fetch; dismissing and swapping an open card are
 * immediate. A touch-raised glance also shows at once, since a tap states its
 * intent. The timer is injectable so the debounce is testable without real time.
 */

import { createSignal, type Accessor } from "solid-js";

import { WINDOW_SCHEDULER, type Scheduler } from "../data/scheduler.js";
import type { GlanceRequest } from "../org/navigation.jsx";

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
    if (request() !== null || next.gesture === "touch") {
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
