/*
 * Bind the navigation grammar to a column's position in the reading stack. `pin`
 * prunes and appends beside its origin column, `go` replaces the whole path,
 * `glance` delegates. Both committing verbs dismiss any open preview first, and
 * `pin` reveals the index `follow` returns, covering a pin that opened nothing.
 */

import { referenceOf, type LinkTarget, type Navigation } from "../org/navigation.jsx";
import type { GlanceController } from "./glance-controller.js";
import type { ReadingStack } from "./stack.js";

/** `index` is read live, so a column keeps its meaning as the stack changes. */
export function spineNavigation(
  stack: ReadingStack,
  glances: GlanceController,
  index: () => number,
  reveal: (index: number) => void,
): Navigation {
  return {
    glance: (request) => glances.glance(request),
    pin: (target: LinkTarget) => {
      glances.glance(null);
      reveal(stack.follow(index(), referenceOf(target)));
    },
    go: (target: LinkTarget) => {
      glances.glance(null);
      stack.open(referenceOf(target));
    },
  };
}
