import type {
  ExplorationEntry,
  ExploreResult,
  BridgeEvidenceRecord,
} from "../api/types.js";
import { targetForNote, type LinkTarget } from "../org/navigation.jsx";

export interface RelatedRow {
  readonly key: string;
  readonly target: LinkTarget;
  readonly title: string;
  readonly connectors: number;
}

export interface RelatedGroup {
  readonly connector: string;
  readonly rows: readonly RelatedRow[];
}

interface Candidate {
  readonly row: RelatedRow;
  readonly via: readonly BridgeEvidenceRecord[];
}

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

/** Group bridge candidates, excluding notes already in the relation inventory. */
export function relatedGroups(
  result: ExploreResult,
  listed: ReadonlySet<string>,
): RelatedGroup[] {
  // Titles are not unique, so groups use connector identity.
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

export function relatedRowTotal(groups: readonly RelatedGroup[]): number {
  return groups.reduce((sum, group) => sum + group.rows.length, 0);
}

/** Keep the highest-ranked rows without destroying their display groups. */
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
