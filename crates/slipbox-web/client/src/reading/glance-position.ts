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

export interface Span {
  readonly left: number;
  readonly right: number;
}

export interface GlancePlacement {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly above: boolean;
}

const GAP = 8;
const MARGIN = 12;
const MIN_BESIDE_WIDTH = 280;

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(value, Math.max(min, max)));
}

/** Prefer the trailing gutter, then the leading gutter. */
function besideReading(
  width: number,
  viewport: Viewport,
  reading: Span,
): { left: number; width: number } | null {
  if (reading.right <= reading.left) {
    return null;
  }
  const trailing = reading.right + GAP;
  const trailingWidth = Math.min(width, viewport.width - MARGIN - trailing);
  if (trailing >= MARGIN && trailingWidth >= MIN_BESIDE_WIDTH) {
    return { left: trailing, width: trailingWidth };
  }
  const leadingWidth = Math.min(width, reading.left - GAP - MARGIN);
  return leadingWidth >= MIN_BESIDE_WIDTH
    ? { left: reading.left - GAP - leadingWidth, width: leadingWidth }
    : null;
}

export function placeGlance(
  anchor: Rect,
  card: { width: number; height: number },
  viewport: Viewport,
  reading: Span,
): GlancePlacement {
  const beside = besideReading(card.width, viewport, reading);
  if (beside !== null) {
    return {
      left: beside.left,
      top: clamp(anchor.top, MARGIN, viewport.height - card.height - MARGIN),
      width: beside.width,
      above: false,
    };
  }

  const width = Math.min(card.width, Math.max(0, viewport.width - 2 * MARGIN));

  const fitsBelow = anchor.bottom + GAP + card.height + MARGIN <= viewport.height;
  const above = !fitsBelow && anchor.top - GAP - card.height >= MARGIN;
  const preferred = above ? anchor.top - GAP - card.height : anchor.bottom + GAP;

  const top = clamp(preferred, MARGIN, viewport.height - card.height - MARGIN);
  const left = clamp(anchor.left, MARGIN, viewport.width - width - MARGIN);

  return { left, top, width, above };
}
