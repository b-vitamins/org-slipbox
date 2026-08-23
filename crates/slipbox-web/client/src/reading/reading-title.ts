import { ApiError } from "../api/client.js";
import { isPinned, type SpineMetrics } from "./spine-geometry.js";

export type ColumnTitle =
  | { readonly title: string }
  | { readonly error: unknown };

/** Return the first unpinned column, or the newest column for an entire stack. */
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

export function columnTitle(known: ColumnTitle | undefined): string | undefined {
  if (known === undefined) {
    return undefined;
  }
  return "title" in known ? known.title : unresolvedTitle(known.error);
}

export function unresolvedTitle(error: unknown): string {
  return error instanceof ApiError && error.isNotFound
    ? "Note not found"
    : "Unavailable";
}
