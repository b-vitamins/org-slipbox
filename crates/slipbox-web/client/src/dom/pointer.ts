/*
 * Whether a pointer can rest on a link without pressing it, which is what the
 * hover-preview rung of the navigation grammar requires.
 *
 * A device may carry several pointers at once, so a gesture's own `pointerType`
 * is the finest scope for the question; `hover: none` reports only the primary
 * pointer and is the fallback where a gesture names none.
 */

/** True when the reader's primary pointer can rest on a link without pressing. */
export function pointerCanHover(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return true;
  }
  return !window.matchMedia("(hover: none)").matches;
}

/**
 * True when the pointer that delivered a gesture can rest on a link without
 * pressing it. A missing or unrecognized `pointerType` (a keyboard activation, a
 * plain mouse event, a pointer kind postdating this code) falls back to the
 * primary pointer's report.
 */
export function gestureCanHover(pointerType: string | null | undefined): boolean {
  if (pointerType === "mouse" || pointerType === "pen") {
    return true;
  }
  if (pointerType === "touch") {
    return false;
  }
  return pointerCanHover();
}
