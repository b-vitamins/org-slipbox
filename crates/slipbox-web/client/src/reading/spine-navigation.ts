import { referenceOf, type LinkTarget, type Navigation } from "../org/navigation.jsx";
import type { GlanceController } from "./glance-controller.js";
import { encodeStack, reduceReadOn, type ReadingStack } from "./stack.js";

/** Bind link navigation to a column whose index may change with the stack. */
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

export interface FilingMove {
  readonly address: (target: string) => string;
  readonly open: (target: string) => void;
}

export function spineFilingMove(
  stack: ReadingStack,
  glances: GlanceController,
  index: () => number,
  reveal: (index: number) => void,
): FilingMove {
  return {
    address: (target) =>
      encodeStack(reduceReadOn(stack.keys(), index(), target).keys),
    open: (target) => {
      glances.glance(null);
      reveal(stack.readOn(index(), target));
    },
  };
}
