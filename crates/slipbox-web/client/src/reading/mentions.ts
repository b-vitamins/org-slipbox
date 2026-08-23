import type {
  ContentSegment,
  UnlinkedReferenceRecord,
  UnlinkedReferencesResult,
} from "../api/types.js";
import { excerptRuns, type ExcerptRun } from "../entry/excerpt.js";
import { targetForNote, type LinkTarget } from "../org/navigation.jsx";
import { RELATION_PREVIEW_CHARS } from "./relations.js";

export interface MentionRow {
  readonly key: string;
  readonly target: LinkTarget;
  readonly title: string;
  readonly preview: readonly ExcerptRun[];
}

const CONTEXT_BEFORE = 24;
const ELLIPSIS = "…";

function sitsAt(
  line: readonly string[],
  run: readonly string[],
  index: number,
): boolean {
  return (
    index >= 0 && run.every((char, offset) => line[index + offset] === char)
  );
}

/** Prefer the scanner's exact character column when text repeats. */
function matchStart(
  line: readonly string[],
  run: readonly string[],
  col: number,
): number | null {
  const stated = col - 1;
  if (sitsAt(line, run, stated)) {
    return stated;
  }
  for (let index = 0; index < line.length; index += 1) {
    if (sitsAt(line, run, index)) {
      return index;
    }
  }
  return null;
}

function previewSegments(record: UnlinkedReferenceRecord): ContentSegment[] {
  const line = [...record.preview];
  const run = [...record.matched_text];
  const at = matchStart(line, run, record.col);
  const from = at === null ? 0 : Math.max(0, at - CONTEXT_BEFORE);
  // Always retain the complete match, even when it exceeds the preview bound.
  const to = Math.max(
    Math.min(line.length, from + RELATION_PREVIEW_CHARS),
    at === null ? 0 : at + run.length,
  );
  const opener = from > 0 ? ELLIPSIS : "";
  const closer = to < line.length ? ELLIPSIS : "";
  if (at === null) {
    return [{ text: `${line.slice(0, to).join("")}${closer}`, matched: false }];
  }
  return [
    { text: `${opener}${line.slice(from, at).join("")}`, matched: false },
    { text: run.join(""), matched: true },
    { text: `${line.slice(at + run.length, to).join("")}${closer}`, matched: false },
  ].filter((segment) => segment.text.length > 0);
}

/** Deduplicate mentions by owning note while preserving scan order. */
export function mentionRows(
  result: UnlinkedReferencesResult,
  listed: ReadonlySet<string>,
): MentionRow[] {
  const rows: MentionRow[] = [];
  const placed = new Set<string>();

  for (const record of result.unlinked_references) {
    const note = record.source_note ?? record.source_anchor;
    if (listed.has(note.node_key) || placed.has(note.node_key)) {
      continue;
    }
    placed.add(note.node_key);
    rows.push({
      key: note.node_key,
      target: targetForNote(note.node_key, note.explicit_id),
      title: note.title,
      preview: excerptRuns(previewSegments(record)),
    });
  }

  return rows;
}
