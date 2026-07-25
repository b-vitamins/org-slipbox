/*
 * Project a bounded neighborhood into ranked distance rings. The endpoint sends
 * a flat node list tagged with hop distance plus the edges between them. Rings
 * drop the origin and any excluded key, and rank each ring by how many of the
 * ring inside it a member is reached through; ties keep the server's order.
 */

import type { LinkTarget } from "../org/navigation.jsx";
import type { Neighborhood, NodeRecord } from "../api/types.js";

export interface RingMember {
  readonly key: string;
  readonly target: LinkTarget;
  readonly title: string;
  /**
   * The nearer notes this one is reached through, in the server's order. Empty
   * for the first ring, whose only nearer note is the origin.
   */
  readonly via: string[];
}

/** Members are ordered nearest-connected first. */
export interface DistanceRing {
  readonly distance: number;
  readonly members: RingMember[];
}

/** Prefer an id target (stable across edits), else fall back to the key. */
function targetFor(node: NodeRecord): LinkTarget {
  return node.explicit_id
    ? { id: node.explicit_id, target: `id:${node.explicit_id}` }
    : { id: null, target: node.node_key };
}

/** Which notes each note is linked to or from, by key. Direction is dropped. */
function adjacency(neighborhood: Neighborhood): Map<string, Set<string>> {
  const links = new Map<string, Set<string>>();
  const join = (from: string, to: string): void => {
    const held = links.get(from);
    if (held) {
      held.add(to);
    } else {
      links.set(from, new Set([to]));
    }
  };
  for (const edge of neighborhood.edges) {
    join(edge.source, edge.target);
    join(edge.target, edge.source);
  }
  return links;
}

/**
 * Group a neighborhood's nodes into ascending distance rings, dropping the
 * origin and any node in `exclude` (by node key).
 *
 * Within a ring, members carrying more connections to the ring inside them come
 * first; equally connected members keep the order the server sent.
 */
export function distanceRings(
  neighborhood: Neighborhood,
  exclude: ReadonlySet<string> = new Set(),
): DistanceRing[] {
  const distances = new Map(
    neighborhood.nodes.map((entry) => [entry.node.node_key, entry.distance]),
  );
  const titles = new Map(
    neighborhood.nodes.map((entry) => [entry.node.node_key, entry.node.title]),
  );
  const links = adjacency(neighborhood);
  // The server's order, so connectors and equally ranked members sort
  // deterministically rather than in `Set` insertion order.
  const rank = new Map(
    neighborhood.nodes.map((entry, index) => [entry.node.node_key, index]),
  );

  const nearerThan = (key: string, distance: number): string[] =>
    [...(links.get(key) ?? [])]
      .filter((other) => distances.get(other) === distance - 1)
      .sort((a, b) => (rank.get(a) ?? 0) - (rank.get(b) ?? 0));

  const byDistance = new Map<number, RingMember[]>();
  const connectionsOf = new Map<string, number>();
  for (const entry of neighborhood.nodes) {
    if (entry.distance === 0 || exclude.has(entry.node.node_key)) {
      continue;
    }
    const key = entry.node.node_key;
    const nearer = nearerThan(key, entry.distance);
    connectionsOf.set(key, nearer.length);
    const member: RingMember = {
      key,
      target: targetFor(entry.node),
      title: entry.node.title,
      via:
        entry.distance <= 1
          ? []
          : nearer.map((other) => titles.get(other) ?? other),
    };
    const ring = byDistance.get(entry.distance);
    if (ring) {
      ring.push(member);
    } else {
      byDistance.set(entry.distance, [member]);
    }
  }

  return [...byDistance.entries()]
    .sort(([a], [b]) => a - b)
    .map(([distance, members]) => ({
      distance,
      // Sort is stable, so equally connected members hold the server's order.
      members: members
        .slice()
        .sort(
          (a, b) =>
            (connectionsOf.get(b.key) ?? 0) - (connectionsOf.get(a.key) ?? 0),
        ),
    }));
}

export function ringPopulation(rings: readonly DistanceRing[]): number {
  return rings.reduce((sum, ring) => sum + ring.members.length, 0);
}

/**
 * The nearest `limit` members of each ring, and how many that leaves unshown.
 *
 * The cut relies on `distanceRings` having ranked the members already.
 */
export function nearestOf(
  rings: readonly DistanceRing[],
  limit: number,
): { rings: DistanceRing[]; hidden: number } {
  const nearest = rings.map((ring) => ({
    distance: ring.distance,
    members: ring.members.slice(0, limit),
  }));
  return { rings: nearest, hidden: ringPopulation(rings) - ringPopulation(nearest) };
}
