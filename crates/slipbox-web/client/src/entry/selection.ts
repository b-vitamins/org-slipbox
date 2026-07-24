/*
 * The pure active-option cursor for a keyboard-driven listbox: `null` for no
 * highlight, cyclic wrap at both ends. Home and End are absent because in a
 * combobox they move the text cursor, not the list.
 */

export type Movement = "next" | "previous";

/**
 * The next highlighted index after `movement` over a list of `count` options,
 * or `null` when the list is empty.
 */
export function moveSelection(
  current: number | null,
  movement: Movement,
  count: number,
): number | null {
  if (count === 0) {
    return null;
  }
  if (movement === "next") {
    return current === null ? 0 : (current + 1) % count;
  }
  return current === null ? count - 1 : (current - 1 + count) % count;
}

/**
 * The index of the option `marked` names, or null if the list does not hold it.
 *
 * The highlight is resolved through option identity, not a row number, so a list
 * re-read that still holds the option keeps it highlighted wherever it now sits.
 */
export function markedIndex<T>(
  marked: string | null,
  options: readonly T[],
  identity: (option: T) => string,
): number | null {
  if (marked === null) {
    return null;
  }
  const index = options.findIndex((option) => identity(option) === marked);
  return index === -1 ? null : index;
}
