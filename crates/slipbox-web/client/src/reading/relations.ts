/*
 * Project a note's fetched relations into the rows the footer renders: one per
 * distinct related note per direction, deduplicated by target note with the
 * first occurrence winning, so the server's order is preserved. The server sends
 * each preview as raw Org source, which is parsed into preview prose.
 */

import type { NoteContext } from "../api/types.js";
import { targetForNote, type LinkTarget } from "../org/navigation.jsx";
import { parseInline } from "../org/parse-inline.js";
import { inlinePreview } from "../org/prose.js";
import type { Inline } from "../org/types.js";

export interface RelationRow {
  /** The related note's slipbox key, the row's dedup identity. */
  readonly key: string;
  readonly target: LinkTarget;
  readonly title: string;
  /** One line of the linking context as preview prose; may be empty. */
  readonly preview: readonly Inline[];
}

function previewProse(raw: string): readonly Inline[] {
  return inlinePreview(parseInline(raw));
}

function dedupe(rows: RelationRow[]): RelationRow[] {
  const seen = new Set<string>();
  const kept: RelationRow[] = [];
  for (const row of rows) {
    if (!seen.has(row.key)) {
      seen.add(row.key);
      kept.push(row);
    }
  }
  return kept;
}

export function forwardRelations(context: NoteContext): RelationRow[] {
  return dedupe(
    context.forward_links.map((link) => ({
      key: link.destination_note.node_key,
      target: targetForNote(
        link.destination_note.node_key,
        link.destination_note.explicit_id,
      ),
      title: link.destination_note.title,
      preview: previewProse(link.preview),
    })),
  );
}

export function backwardRelations(context: NoteContext): RelationRow[] {
  return dedupe(
    context.backlinks.map((link) => ({
      key: link.source_note.node_key,
      target: targetForNote(
        link.source_note.node_key,
        link.source_note.explicit_id,
      ),
      title: link.source_note.title,
      preview: previewProse(link.preview),
    })),
  );
}
