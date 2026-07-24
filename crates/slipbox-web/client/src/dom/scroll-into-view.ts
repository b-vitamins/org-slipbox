/*
 * Keep a listbox's highlighted option visible.
 *
 * A listbox drives its cursor through `aria-activedescendant` rather than focus,
 * so the browser never scrolls the highlight into view the way it does for a
 * focused element.
 */

import { prefersReducedMotion } from "./reduced-motion.js";

/**
 * Scroll the option named by `id` into view, if the document holds one. The id
 * is the one `aria-activedescendant` carries, which the listbox contract
 * requires to be unique. A missing id is a no-op.
 */
export function revealOption(id: string | undefined): void {
  const option = id === undefined ? null : document.getElementById(id);
  // Absent in jsdom, where there is no layout to scroll in the first place.
  if (!option || typeof option.scrollIntoView !== "function") {
    return;
  }
  option.scrollIntoView({
    // `nearest` leaves an option already in view where it is and brings one past
    // the edge only to the edge, so the list does not lurch a box per keystroke.
    block: "nearest",
    behavior: prefersReducedMotion() ? "auto" : "smooth",
  });
}
