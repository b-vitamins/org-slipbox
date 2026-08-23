import { createRoot } from "solid-js";
import { describe, expect, it } from "vitest";

import { createGlanceController } from "./glance-controller.js";
import type { Scheduler } from "../data/scheduler.js";
import type { GlanceGesture, GlanceRequest } from "../org/navigation.jsx";

/** A manual scheduler: callbacks fire only when `flush` is called. */
function manualScheduler(): Scheduler & { flush: () => void; pending: () => number } {
  const jobs = new Map<number, () => void>();
  let nextHandle = 1;
  return {
    set: (fn) => {
      const handle = nextHandle++;
      jobs.set(handle, fn);
      return handle;
    },
    clear: (handle) => {
      jobs.delete(handle);
    },
    flush: () => {
      for (const fn of [...jobs.values()]) {
        fn();
      }
      jobs.clear();
    },
    pending: () => jobs.size,
  };
}

/** A glance request carrying a distinguishable id, without a real DOM node. */
function requestFor(id: string, gesture: GlanceGesture = "hover"): GlanceRequest {
  return {
    target: { id, target: `id:${id}` },
    origin: {} as HTMLElement,
    gesture,
    pin: () => {},
    go: () => {},
    dismiss: () => {},
  };
}

describe("createGlanceController", () => {
  it("delays showing a preview until the intent window elapses", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const controller = createGlanceController(120, scheduler);

      controller.glance(requestFor("a"));
      expect(controller.request()).toBeNull(); // not shown yet
      expect(scheduler.pending()).toBe(1);

      scheduler.flush();
      expect(controller.request()?.target.id).toBe("a");
      dispose();
    });
  });

  it("dismisses immediately without waiting", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const controller = createGlanceController(120, scheduler);

      controller.glance(requestFor("a"));
      scheduler.flush();
      controller.glance(null);
      expect(controller.request()).toBeNull();
      dispose();
    });
  });

  it("swaps an already-open preview immediately, with no new delay", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const controller = createGlanceController(120, scheduler);

      controller.glance(requestFor("a"));
      scheduler.flush();
      expect(controller.request()?.target.id).toBe("a");

      controller.glance(requestFor("b"));
      expect(controller.request()?.target.id).toBe("b"); // no flush needed
      expect(scheduler.pending()).toBe(0);
      dispose();
    });
  });

  // The window exists to tell a deliberate hover from a cursor passing over. A
  // tap is already deliberate, and it is the whole gesture — nothing follows it —
  // so waiting would read as the tap having been missed.
  it("shows a tapped preview at once, without the intent window", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const controller = createGlanceController(120, scheduler);

      controller.glance(requestFor("a", "touch"));
      expect(controller.request()?.target.id).toBe("a");
      expect(scheduler.pending()).toBe(0);
      dispose();
    });
  });

  it("cancels a pending show when dismissed before intent elapses", () => {
    createRoot((dispose) => {
      const scheduler = manualScheduler();
      const controller = createGlanceController(120, scheduler);

      controller.glance(requestFor("a"));
      controller.glance(null);
      expect(scheduler.pending()).toBe(0);
      scheduler.flush();
      expect(controller.request()).toBeNull();
      dispose();
    });
  });
});
