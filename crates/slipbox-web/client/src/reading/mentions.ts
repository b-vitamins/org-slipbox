/*
 * Project an unlinked-reference scan into the footer's mention rows: notes that
 * name this one in their own prose without linking to it.
 *
 * The scan reports one record per occurrence, ordered by indexed file path and
 * then by position in the file. That order is kept, and the first occurrence in a
 * note is the one row it gets: what a reader acts on is the note, not the offset.
 *
 * The matched text is not carried beside the title. It rides inside the preview
 * as a flagged run, spliced and reduced by the same machinery a search excerpt
 * uses, so a marked run reads alike wherever the surface draws one.
 */

import type {
  ContentSegment,
  UnlinkedReferenceRecord,
  UnlinkedReferencesResult,
} from "../api/types.js";
import { excerptRuns, type ExcerptRun } from "../entry/excerpt.js";
import { targetForNote, type LinkTarget } from "../org/navigation.jsx";
import { RELATION_PREVIEW_CHARS } from "./relations.js";

export interface MentionRow {
  /** The mentioning note's slipbox key, the row's dedup identity. */
  readonly key: string;
  readonly target: LinkTarget;
  readonly title: string;
  /** The mentioning line as preview prose, the matched run flagged. */
  readonly preview: readonly ExcerptRun[];
}

/**
 * Characters of the line kept ahead of the match. A row paints one clipped line,
 * so a mention late in a long line would sit past the ellipsis with nothing to
 * scroll; the line is wound forward to the match instead, and the cut marked.
 */
const CONTEXT_BEFORE = 24;

/** What a cut edge is marked with, as the server marks its own elisions. */
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

/**
 * Where the matched run sits in the line, counted in characters, or null when the
 * line does not hold it. The stated column is read first: the same text may stand
 * earlier in the line inside a link the scan passed over.
 */
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

/**
 * The window of the mentioning line a row carries, flagged where the scan
 * matched. Bounded like a link row's preview, since the clip is paint only and
 * the whole line would otherwise be read out in full.
 */
function previewSegments(record: UnlinkedReferenceRecord): ContentSegment[] {
  // Characters, not code units: the scan counts the former.
  const line = [...record.preview];
  const run = [...record.matched_text];
  const at = matchStart(line, run, record.col);
  const from = at === null ? 0 : Math.max(0, at - CONTEXT_BEFORE);
  // The matched run is never cut short: it is what the row is for.
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

/**
 * One row per mentioning node, the scan's order kept.
 *
 * @param result the answer to the unlinked-reference scan
 * @param listed keys the directed inventory already prints, which are dropped
 */
export function mentionRows(
  result: UnlinkedReferencesResult,
  listed: ReadonlySet<string>,
): MentionRow[] {
  const rows: MentionRow[] = [];
  const placed = new Set<string>();

  for (const record of result.unlinked_references) {
    // The scan reports the node the line sits in, down to a heading carrying no
    // id, and names the note holding it beside. The note is what the inventory
    // lists and what a reader acts on, so it is the row's whole identity; the
    // anchor stands in for it only where the payload carries no note.
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
