import type { NoteContext } from "../api/types.js";
import { targetForNote, type LinkTarget } from "../org/navigation.jsx";
import { parseInline } from "../org/parse-inline.js";
import { inlinePreview } from "../org/prose.js";
import type { Inline } from "../org/types.js";

export type RelationDirection = "out" | "in" | "both";

export interface RelationRow {
  readonly key: string;
  readonly target: LinkTarget;
  readonly title: string;
  readonly direction: RelationDirection;
  readonly preview: readonly Inline[];
}

// Bound accessible text as well as the visual preview.
export const RELATION_PREVIEW_CHARS = 120;

function previewProse(raw: string): readonly Inline[] {
  return inlinePreview(parseInline(raw), RELATION_PREVIEW_CHARS);
}

/** Deduplicate related notes while preserving server order. */
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

export function shownInDirection(
  rows: readonly RelationRow[],
  direction: "out" | "in",
): number {
  return rows.filter(
    (row) => row.direction === direction || row.direction === "both",
  ).length;
}
