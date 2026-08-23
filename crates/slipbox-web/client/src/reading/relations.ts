/*
 * Project a note's fetched relations into the rows the footer renders: one row
 * per distinct related note, carrying the direction its links run in. Rows keep
 * the order the notes first appear in, forward links first, so the server's
 * ordering survives.
 *
 * A preview only ever quotes the other note, so it comes from a backlink record
 * and a row the note being read only links out to carries none: that record's
 * preview is the line already on screen above the footer. The server sends each
 * preview as raw Org source, which is parsed into preview prose.
 */

import type { NoteContext } from "../api/types.js";
import { targetForNote, type LinkTarget } from "../org/navigation.jsx";
import { parseInline } from "../org/parse-inline.js";
import { inlinePreview } from "../org/prose.js";
import type { Inline } from "../org/types.js";

/** Which way the links between the note being read and a related one run. */
export type RelationDirection = "out" | "in" | "both";

export interface RelationRow {
  /** The related note's slipbox key, the row's dedup identity. */
  readonly key: string;
  readonly target: LinkTarget;
  readonly title: string;
  readonly direction: RelationDirection;
  /** The other note's line as preview prose; empty on a forward-only row. */
  readonly preview: readonly Inline[];
}

function previewProse(raw: string): readonly Inline[] {
  return inlinePreview(parseInline(raw));
}

/** One row per related note, direction marked, first appearance winning. */
export function relationRows(context: NoteContext): RelationRow[] {
  const rows: RelationRow[] = [];
  const placed = new Map<string, number>();

  for (const link of context.forward_links) {
    const note = link.destination_note;
    if (placed.has(note.node_key)) {
      continue;
    }
    placed.set(note.node_key, rows.length);
    rows.push({
      key: note.node_key,
      target: targetForNote(note.node_key, note.explicit_id),
      title: note.title,
      direction: "out",
      preview: [],
    });
  }

  for (const link of context.backlinks) {
    const note = link.source_note;
    const at = placed.get(note.node_key);
    if (at === undefined) {
      placed.set(note.node_key, rows.length);
      rows.push({
        key: note.node_key,
        target: targetForNote(note.node_key, note.explicit_id),
        title: note.title,
        direction: "in",
        preview: previewProse(link.preview),
      });
      continue;
    }
    const held = rows[at];
    // Only a row placed by a forward link is upgraded; a second backlink from a
    // note already listed inbound leaves the first occurrence's preview alone.
    if (held?.direction === "out") {
      rows[at] = {
        ...held,
        direction: "both",
        preview: previewProse(link.preview),
      };
    }
  }

  return rows;
}

/** Rows a direction reaches, which its payload total is measured against. */
export function shownInDirection(
  rows: readonly RelationRow[],
  direction: "out" | "in",
): number {
  return rows.filter(
    (row) => row.direction === direction || row.direction === "both",
  ).length;
}
