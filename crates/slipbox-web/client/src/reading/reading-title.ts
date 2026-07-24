/*
 * The reading position's identity, for the browser tab. Resolves a reference
 * (`id:<uuid>` or a raw slipbox key) through `/api/node`, which reads identity
 * only: the column already fetches the body, and the tab needs the title.
 */

import { ApiError, client } from "../api/client.js";

/** The reference whose title names the tab: the frontmost (rightmost) note. */
export function frontmostReference(keys: readonly string[]): string | undefined {
  return keys.length === 0 ? undefined : keys[keys.length - 1];
}

export async function resolveNoteTitle(reference: string): Promise<string> {
  const node = reference.startsWith("id:")
    ? await client.nodeById(reference.slice(3))
    : await client.nodeByKey(reference);
  return node.title;
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
