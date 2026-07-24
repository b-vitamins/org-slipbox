/*
 * `prefers-reduced-motion` for scripted scrolling. The token overrides already
 * collapse CSS transitions, but `scrollTo({ behavior: "smooth" })` bypasses CSS
 * and animates regardless, so the preference is read in script here.
 */

/** True when the reader has asked the platform to reduce motion. */
export function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** The scroll behavior for a scripted reveal, respecting the preference. */
export function scrollBehavior(): ScrollBehavior {
  return prefersReducedMotion() ? "auto" : "smooth";
}
