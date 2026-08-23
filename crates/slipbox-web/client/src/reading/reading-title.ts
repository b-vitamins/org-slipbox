/*
 * What the browser tab is called while a stack is being read. One column names
 * the tab, and every column already read its own note, so nothing here fetches:
 * the spine hands in the states its geometry computed and the title the named
 * column resolved.
 */

import { ApiError } from "../api/client.js";
import { isPinned, type SpineMetrics } from "./spine-geometry.js";

/** What a column knows its note as: the title it read, or the read's failure. */
export type ColumnTitle =
  | { readonly title: string }
  | { readonly error: unknown };

/**
 * Which column the tab is named after, or undefined for an empty stack.
 *
 * The first column the ladder has not pinned is the one being read: every column
 * before it has been pulled off its place in the flow and stands in the ladder,
 * and the reader has scrolled no further than the first that has not. A column
 * resting is not that test - the columns past the reading position rest too,
 * having never been reached - so it is the pinning that is read, off the same
 * geometry the states are computed from.
 *
 * `entire` says the spine stands in the viewport whole - a stack shorter than the
 * frame, or the narrow layout, which stacks the columns vertically and pins none
 * of them. Then no column is the reading position, and the frontmost is the one
 * the reader just opened. That is also the fallback where every column is pinned,
 * which keeps the tab named rather than blank.
 */
export function revealedColumn(
  count: number,
  scrollLeft: number,
  metrics: SpineMetrics,
  entire: boolean,
): number | undefined {
  if (count === 0) {
    return undefined;
  }
  const frontmost = count - 1;
  if (entire) {
    return frontmost;
  }
  for (let index = 0; index < count; index += 1) {
    if (!isPinned(index, scrollLeft, metrics)) {
      return index;
    }
  }
  return frontmost;
}

/**
 * The tab's label for what a column knows: the note's title, or how its read
 * failed. A column still reading knows neither, and the tab falls back to the
 * product name rather than holding the title of a note left behind.
 */
export function columnTitle(known: ColumnTitle | undefined): string | undefined {
  if (known === undefined) {
    return undefined;
  }
  return "title" in known ? known.title : unresolvedTitle(known.error);
}

/**
 * What the tab is called when the reading position cannot be named. The
 * daemon's own message is not used: it is written for a page, not a tab strip.
 */
export function unresolvedTitle(error: unknown): string {
  return error instanceof ApiError && error.isNotFound
    ? "Note not found"
    : "Unavailable";
}
