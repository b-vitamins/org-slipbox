/*
 * Spine geometry: fixed-width columns in a horizontal scroller, each `sticky`.
 * A column pins at `sliver * index`, capped at `gutterDepth`. State is read off
 * those offsets rather than from separate thresholds, so a column is obscured
 * exactly when the column ahead of it or the viewport edge leaves it a sliver.
 */

export type ColumnState = "resting" | "overlay" | "obscured";

/** All in px, measured off the live scroll container. */
export interface SpineMetrics {
  readonly columnWidth: number;
  readonly sliver: number;
  readonly viewport: number;
  readonly scrollWidth: number;
}

/**
 * How many slivers the pinned ladder may hold: as many as fit beside one fully
 * open column.
 *
 * Without this cap a deep stack's ladder grows wider than the viewport and every
 * column pins past the right edge, leaving no readable column at any offset.
 */
export function gutterDepth(metrics: SpineMetrics): number {
  const room = metrics.viewport - metrics.columnWidth;
  if (room <= 0 || metrics.sliver <= 0) {
    return 0;
  }
  return Math.floor(room / metrics.sliver);
}

/**
 * The sticky left offset (px) column `index` pins at.
 *
 * A function of the metrics alone, not of the scroll offset: this is the
 * column's CSS `left`, and `position: sticky` turns it into a pin.
 */
export function columnOffset(index: number, metrics: SpineMetrics): number {
  return Math.min(index, gutterDepth(metrics)) * metrics.sliver;
}

function columnLeft(
  index: number,
  scrollLeft: number,
  metrics: SpineMetrics,
): number {
  return Math.max(index * metrics.columnWidth - scrollLeft, columnOffset(index, metrics));
}

/** Whether a column has reached its sticky offset. */
export function isPinned(
  index: number,
  scrollLeft: number,
  metrics: SpineMetrics,
): boolean {
  return columnOffset(index, metrics) > index * metrics.columnWidth - scrollLeft;
}

export function columnState(
  index: number,
  count: number,
  scrollLeft: number,
  metrics: SpineMetrics,
): ColumnState {
  // A zero viewport is an unmeasured container, not a covered column: judging
  // against it would obscure every column before the first layout.
  if (metrics.viewport <= 0) {
    return "resting";
  }
  const left = columnLeft(index, scrollLeft, metrics);
  const ahead =
    index + 1 < count
      ? columnLeft(index + 1, scrollLeft, metrics)
      : Number.POSITIVE_INFINITY;
  const shown =
    Math.min(left + metrics.columnWidth, ahead, metrics.viewport) - Math.max(left, 0);
  if (shown <= metrics.sliver) {
    return "obscured";
  }
  const floating =
    isPinned(index, scrollLeft, metrics) ||
    (index > 0 && isPinned(index - 1, scrollLeft, metrics));
  return floating ? "overlay" : "resting";
}

/**
 * Every column rests in the narrow layout: the sliver is hidden by CSS there and
 * an `obscured` column hides its body, so a collapsed column would vanish.
 */
export function columnStates(
  count: number,
  scrollLeft: number,
  metrics: SpineMetrics,
  narrow: boolean,
): ColumnState[] {
  if (narrow) {
    return Array.from({ length: count }, () => "resting");
  }
  return Array.from({ length: count }, (_, index) =>
    columnState(index, count, scrollLeft, metrics),
  );
}

export interface VerticalScrollport {
  /** The scrollport's top edge in viewport coordinates. */
  readonly top: number;
  readonly scrollTop: number;
}

/**
 * The vertical scroll offset that brings a stacked column's top edge to the top
 * of the scrollport, for the narrow layout.
 *
 * `columnTop` must be a viewport coordinate, not `offsetTop`: `offsetTop` is
 * measured from the nearest positioned ancestor, which in the narrow layout is
 * the page, so scrolling by it overshoots by the chrome above the spine.
 */
export function verticalRevealTop(
  scrollport: VerticalScrollport,
  columnTop: number,
): number {
  return Math.max(0, scrollport.scrollTop + (columnTop - scrollport.top));
}

/** The stacked column at the scrollport's leading edge. */
export function verticalRevealedColumn(
  scrollportTop: number,
  columnTops: readonly number[],
): number | undefined {
  if (columnTops.length === 0) {
    return undefined;
  }
  let revealed = 0;
  for (let index = 1; index < columnTops.length; index += 1) {
    if (columnTops[index]! > scrollportTop + 1) {
      break;
    }
    revealed = index;
  }
  return revealed;
}

/**
 * The scroll offset that brings column `index` into view, centering it when the
 * spine overflows the viewport. Null when the spine fits and no scroll is needed.
 *
 * The inset is floored at `columnOffset`, or the offset meant to reveal the
 * column would park it under the pinned slivers.
 */
export function scrollTargetFor(
  index: number,
  metrics: SpineMetrics,
): number | null {
  if (metrics.scrollWidth <= metrics.viewport) {
    return null;
  }
  const centered = (metrics.viewport - metrics.columnWidth) / 2;
  const inset = Math.max(centered, columnOffset(index, metrics));
  return Math.max(
    0,
    Math.min(index * metrics.columnWidth - inset, metrics.scrollWidth - metrics.viewport),
  );
}
