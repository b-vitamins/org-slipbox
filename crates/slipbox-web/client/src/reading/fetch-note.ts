/*
 * Resolve a note reference (`id:<uuid>` or a raw slipbox key) to its reading
 * context, and record the resolved note's identity for the reading stack. The
 * server caps every source read, so no `maxLines` buys a whole note: a longer
 * note returns as a prefix with the slice's truncation flags set.
 */

import { client } from "../api/client.js";
import type { NoteContext } from "../api/types.js";
import { noteIdentities } from "./note-identity.js";

/**
 * The largest source read the server will serve, asked for in full.
 *
 * 1000 is `MAX_SOURCE_LINES` in slipbox-core's `source.rs`, restated as the
 * `max_lines` bound in the web crate's routes. Raising it buys nothing.
 */
export const WHOLE_NOTE_MAX_LINES = 1000;

/** Enough lines to build a short glance excerpt without fetching the body. */
export const PREVIEW_MAX_LINES = 40;

/**
 * Relations asked for per direction: 200 is the ceiling the context route
 * admits, and its own default of 25 cuts a well-linked note's backlinks.
 */
export const MAX_RELATIONS_PER_DIRECTION = 200;

export async function fetchNoteContext(
  reference: string,
  maxLines: number,
): Promise<NoteContext> {
  const options = { maxLines, relations: MAX_RELATIONS_PER_DIRECTION };
  const context = reference.startsWith("id:")
    ? await client
        .nodeById(reference.slice(3))
        .then((node) => client.noteContext(node.node_key, options))
    : await client.noteContext(reference, options);
  noteIdentities.learn(context.note);
  return context;
}
