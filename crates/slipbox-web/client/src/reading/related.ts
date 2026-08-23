/*
 * Project a bridges exploration into the footer's ranked-related rows: notes two
 * hops away that the note being read does not link to, each carrying the notes
 * the lens travelled through to reach it.
 *
 * The lens ranks its answer, so the order it sends is kept. Rows are then
 * gathered under their lead connector, in the order those connectors first
 * appear, which names a connector once instead of once per row and leaves the
 * ranking intact inside each group.
 */

import type {
  ExplorationEntry,
  ExploreResult,
  BridgeEvidenceRecord,
} from "../api/types.js";
import { targetForNote, type LinkTarget } from "../org/navigation.jsx";

export interface RelatedRow {
  /** The related note's slipbox key. */
  readonly key: string;
  readonly target: LinkTarget;
  readonly title: string;
  /** Notes the lens reached it through, which is how far from one it stands. */
  readonly connectors: number;
}

export interface RelatedGroup {
  /** The lead connector's title, which the group is named after. */
  readonly connector: string;
  readonly rows: readonly RelatedRow[];
}

/** A candidate's own record: the note reached, and the notes it was reached by. */
interface Candidate {
  readonly row: RelatedRow;
  readonly via: readonly BridgeEvidenceRecord[];
}

/**
 * The candidate an entry describes, or null when the entry is not one. The lens
 * decides an entry's shape, so both the tag and the explanation are read rather
 * than assumed from the section they arrived in.
 */
function candidate(entry: ExplorationEntry): Candidate | null {
  if (entry.kind !== "anchor" || entry.explanation.kind !== "bridge-candidate") {
    return null;
  }
  const via = entry.explanation.via_notes;
  if (via.length === 0) {
    return null;
  }
  const anchor = entry.anchor;
  return {
    row: {
      key: anchor.node_key,
      target: targetForNote(anchor.node_key, anchor.explicit_id),
      title: anchor.title,
      connectors: via.length,
    },
    via,
  };
}

/**
 * Group a bridges answer's candidates under their lead connector.
 *
 * @param result the answer to the `bridges` lens
 * @param listed keys the directed inventory already prints, which are dropped
 */
export function relatedGroups(
  result: ExploreResult,
  listed: ReadonlySet<string>,
): RelatedGroup[] {
  // Keyed on the connector's own note, not its title: two notes may share a
  // title, and merging them would file rows under a connector they never had.
  const groups = new Map<string, { connector: string; rows: RelatedRow[] }>();

  for (const section of result.sections) {
    if (section.kind !== "bridge-candidates") {
      continue;
    }
    for (const entry of section.entries) {
      const found = candidate(entry);
      if (!found || listed.has(found.row.key)) {
        continue;
      }
      const lead = found.via[0]!;
      const held = groups.get(lead.node_key);
      if (held) {
        held.rows.push(found.row);
        continue;
      }
      groups.set(lead.node_key, { connector: lead.title, rows: [found.row] });
    }
  }

  return [...groups.values()].map((group) => ({
    connector: group.connector,
    rows: group.rows,
  }));
}

/**
 * Candidates the answer carries, before a note the footer already lists is
 * dropped from it. Measured against the limit the request asked for, this is
 * what says whether the lens itself cut the answer short.
 */
export function rankedTotal(result: ExploreResult): number {
  let ranked = 0;
  for (const section of result.sections) {
    if (section.kind !== "bridge-candidates") {
      continue;
    }
    for (const entry of section.entries) {
      if (candidate(entry) !== null) {
        ranked += 1;
      }
    }
  }
  return ranked;
}

/** Rows the groups hold between them, which a display bound is measured against. */
export function relatedRowTotal(groups: readonly RelatedGroup[]): number {
  return groups.reduce((sum, group) => sum + group.rows.length, 0);
}

/**
 * The first `limit` rows in ranked order, still grouped. A group left with no
 * row inside the bound is dropped rather than named over nothing.
 *
 * The cut is on the ranking the groups were built from, not on the group
 * boundary: a group gathers rows the lens ranked apart, so filling one group
 * before the next would hold a row reached through one note over a row reached
 * through several. The ranking is the connector count, which is what the lens
 * ranks on, and rows it ranked level are cut in group order - the only order the
 * groups still carry between them. What survives is regrouped as it stood, so
 * both the group order and the order inside a group are the ones handed in.
 */
export function boundedRelated(
  groups: readonly RelatedGroup[],
  limit: number,
): RelatedGroup[] {
  const kept = new Set(
    groups
      .flatMap((group) => [...group.rows])
      .sort((left, right) => right.connectors - left.connectors)
      .slice(0, Math.max(limit, 0)),
  );
  const bounded: RelatedGroup[] = [];
  for (const group of groups) {
    const rows = group.rows.filter((row) => kept.has(row));
    if (rows.length > 0) {
      bounded.push({ connector: group.connector, rows });
    }
  }
  return bounded;
}
