import { createSignal, type Accessor } from "solid-js";

import { WINDOW_SCHEDULER, type Scheduler } from "../data/scheduler.js";
import type { GlanceRequest } from "../org/navigation.jsx";

export interface GlanceController {
  readonly request: Accessor<GlanceRequest | null>;
  readonly glance: (next: GlanceRequest | null) => void;
  readonly cancel: () => void;
}

/** Delay hover previews; touch and replacement requests remain immediate. */
export function createGlanceController(
  intent = 120,
  scheduler: Scheduler = WINDOW_SCHEDULER,
): GlanceController {
  const [request, setRequest] = createSignal<GlanceRequest | null>(null);
  let pending: { handle: number; glance: GlanceRequest } | null = null;

  const clearPending = (): void => {
    if (pending !== null) {
      scheduler.clear(pending.handle);
      pending = null;
    }
  };

  const held = (): GlanceRequest | null => request() ?? pending?.glance ?? null;

  const glance = (next: GlanceRequest | null): void => {
    const dropped = held();
    clearPending();
    if (next === null) {
      setRequest(null);
    } else if (request() !== null || next.gesture === "touch") {
      setRequest(next);
    } else {
      const handle = scheduler.set(() => {
        pending = null;
        setRequest(next);
      }, intent);
      pending = { handle, glance: next };
    }
    if (dropped !== null && dropped !== next) {
      dropped.dropped();
    }
  };

  const cancel = (): void => {
    clearPending();
    setRequest(null);
  };

  return { request, glance, cancel };
}
