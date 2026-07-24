/*
 * Where a glance preview floats relative to the link that raised it. The card
 * prefers to sit just below its link, left-aligned to it, flipping above when it
 * would overflow the bottom and shifting left when it would overflow the right.
 */

/** The subset of `DOMRect` the math needs. */
export interface Rect {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly width: number;
  readonly height: number;
}

export interface Viewport {
  readonly width: number;
  readonly height: number;
}

/** The card's top-left corner, in viewport (fixed-position) coordinates. */
export interface GlancePlacement {
  readonly left: number;
  readonly top: number;
  readonly above: boolean;
}

/** Gap between the link and the card, in px. */
const GAP = 8;
/** Minimum margin the card keeps from any viewport edge, in px. */
const MARGIN = 12;

/** Clamp `value` into `[min, max]`, tolerating an inverted range. */
function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(value, Math.max(min, max)));
}

export function placeGlance(
  anchor: Rect,
  card: { width: number; height: number },
  viewport: Viewport,
): GlancePlacement {
  const fitsBelow = anchor.bottom + GAP + card.height + MARGIN <= viewport.height;
  const above = !fitsBelow && anchor.top - GAP - card.height >= MARGIN;
  const preferred = above ? anchor.top - GAP - card.height : anchor.bottom + GAP;

  // A card taller than the room both above and below its link has no preferred
  // edge on screen, so both axes are clamped into the margins.
  const top = clamp(preferred, MARGIN, viewport.height - card.height - MARGIN);
  const left = clamp(anchor.left, MARGIN, viewport.width - card.width - MARGIN);

  return { left, top, above };
}
