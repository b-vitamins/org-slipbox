import { describe, expect, it } from "vitest";

import { distanceRings, nearestOf, ringPopulation } from "./neighborhood.js";
import type { Neighborhood, NeighborhoodEdge, NodeRecord } from "../api/types.js";

function node(
  key: string,
  title: string,
  explicitId: string | null = null,
): NodeRecord {
  return {
    node_key: key,
    explicit_id: explicitId,
    file_path: "notes/n.org",
    title,
    outline_path: title,
    aliases: [],
    tags: [],
    refs: [],
    todo_keyword: null,
    scheduled_for: null,
    deadline_for: null,
    closed_at: null,
    glossary: false,
    glossary_status: null,
    sr_due: null,
    sr_ease: null,
    sr_interval: null,
    sr_reps: null,
    sr_last: null,
    level: 0,
    line: 1,
    kind: "file",
    file_mtime_ns: 0,
    backlink_count: 0,
    forward_link_count: 0,
  };
}

function neighborhood(
  entries: { node: NodeRecord; distance: number }[],
  truncated = false,
  edges: NeighborhoodEdge[] = [],
): Neighborhood {
  return {
    origin: "notes/self.org::0",
    hops: 2,
    nodes: entries,
    edges,
    truncated,
  };
}

function edge(source: string, target: string): NeighborhoodEdge {
  return { source, target, kind: "forward" };
}

describe("distanceRings", () => {
  it("groups neighbors into ascending rings and drops the origin", () => {
    const rings = distanceRings(
      neighborhood([
        { node: node("notes/self.org::0", "Self"), distance: 0 },
        { node: node("notes/b.org::0", "Beta"), distance: 2 },
        { node: node("notes/a.org::0", "Alpha"), distance: 1 },
        { node: node("notes/c.org::0", "Gamma"), distance: 2 },
      ]),
    );

    expect(rings.map((ring) => ring.distance)).toEqual([1, 2]);
    expect(rings[0]!.members.map((m) => m.title)).toEqual(["Alpha"]);
    expect(rings[1]!.members.map((m) => m.title)).toEqual(["Beta", "Gamma"]);
  });

  it("prefers an id target when a neighbor carries an explicit id", () => {
    const rings = distanceRings(
      neighborhood([
        { node: node("notes/a.org::0", "Alpha", "uuid-1"), distance: 1 },
      ]),
    );
    expect(rings[0]!.members[0]!.target).toEqual({
      id: "uuid-1",
      target: "id:uuid-1",
    });
  });

  it("returns no rings for a neighborhood that is only the origin", () => {
    const rings = distanceRings(
      neighborhood([{ node: node("notes/self.org::0", "Self"), distance: 0 }]),
    );
    expect(rings).toEqual([]);
  });

  it("drops nodes already shown as immediate relations", () => {
    const rings = distanceRings(
      neighborhood([
        { node: node("notes/self.org::0", "Self"), distance: 0 },
        { node: node("notes/a.org::0", "Alpha"), distance: 1 },
        { node: node("notes/b.org::0", "Beta"), distance: 1 },
        { node: node("notes/c.org::0", "Gamma"), distance: 2 },
      ]),
      new Set(["notes/a.org::0", "notes/c.org::0"]),
    );

    // Alpha and Gamma are excluded, so only Beta's ring survives.
    expect(rings.map((ring) => ring.distance)).toEqual([1]);
    expect(rings[0]!.members.map((m) => m.title)).toEqual(["Beta"]);
  });

  it("ranks a ring by how many nearer notes each member is reached through", () => {
    const rings = distanceRings(
      neighborhood(
        [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
          { node: node("notes/b.org::0", "Beta"), distance: 1 },
          { node: node("notes/thin.org::0", "Thin"), distance: 2 },
          { node: node("notes/thick.org::0", "Thick"), distance: 2 },
        ],
        false,
        [
          edge("notes/a.org::0", "notes/thin.org::0"),
          edge("notes/a.org::0", "notes/thick.org::0"),
          edge("notes/b.org::0", "notes/thick.org::0"),
        ],
      ),
    );

    // Thick is reached through Alpha and Beta, Thin through Alpha only.
    expect(rings[1]!.members.map((m) => m.title)).toEqual(["Thick", "Thin"]);
  });

  it("counts a connection whichever way the link points", () => {
    const rings = distanceRings(
      neighborhood(
        [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
          { node: node("notes/b.org::0", "Beta"), distance: 2 },
        ],
        false,
        [edge("notes/b.org::0", "notes/a.org::0")],
      ),
    );

    expect(rings[1]!.members[0]!.via).toEqual(["Alpha"]);
  });

  it("keeps the server's order among equally connected members", () => {
    const rings = distanceRings(
      neighborhood(
        [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
          { node: node("notes/first.org::0", "First"), distance: 2 },
          { node: node("notes/second.org::0", "Second"), distance: 2 },
        ],
        false,
        [
          edge("notes/a.org::0", "notes/first.org::0"),
          edge("notes/a.org::0", "notes/second.org::0"),
        ],
      ),
    );

    expect(rings[1]!.members.map((m) => m.title)).toEqual(["First", "Second"]);
  });

  it("explains a far member by its connectors and the near ring not at all", () => {
    const rings = distanceRings(
      neighborhood(
        [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
          { node: node("notes/b.org::0", "Beta"), distance: 1 },
          { node: node("notes/far.org::0", "Far"), distance: 2 },
        ],
        false,
        [
          edge("notes/self.org::0", "notes/a.org::0"),
          edge("notes/a.org::0", "notes/far.org::0"),
          edge("notes/b.org::0", "notes/far.org::0"),
        ],
      ),
    );

    expect(rings[0]!.members.map((m) => m.via)).toEqual([[], []]);
    expect(rings[1]!.members[0]!.via).toEqual(["Alpha", "Beta"]);
  });

  it("still explains a member through a note kept out of the rings", () => {
    const rings = distanceRings(
      neighborhood(
        [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
          { node: node("notes/far.org::0", "Far"), distance: 2 },
        ],
        false,
        [edge("notes/a.org::0", "notes/far.org::0")],
      ),
      new Set(["notes/a.org::0"]),
    );

    expect(rings.map((ring) => ring.distance)).toEqual([2]);
    expect(rings[0]!.members[0]!.via).toEqual(["Alpha"]);
  });
});

describe("nearestOf", () => {
  const wide = (count: number): Neighborhood =>
    neighborhood([
      { node: node("notes/self.org::0", "Self"), distance: 0 },
      ...Array.from({ length: count }, (_, index) => ({
        node: node(`notes/n${index}.org::0`, `Note ${index}`),
        distance: 1,
      })),
    ]);

  it("keeps the nearest members of each ring and counts the rest", () => {
    const { rings, hidden } = nearestOf(distanceRings(wide(12)), 5);

    expect(rings[0]!.members.map((m) => m.title)).toEqual([
      "Note 0",
      "Note 1",
      "Note 2",
      "Note 3",
      "Note 4",
    ]);
    expect(hidden).toBe(7);
  });

  it("hides nothing when every ring already fits", () => {
    const rings = distanceRings(wide(3));
    const bounded = nearestOf(rings, 5);

    expect(bounded.hidden).toBe(0);
    expect(ringPopulation(bounded.rings)).toBe(3);
  });

  it("counts what is left across every ring at once", () => {
    const rings = distanceRings(
      neighborhood([
        { node: node("notes/self.org::0", "Self"), distance: 0 },
        { node: node("notes/a.org::0", "Alpha"), distance: 1 },
        { node: node("notes/b.org::0", "Beta"), distance: 1 },
        { node: node("notes/c.org::0", "Gamma"), distance: 2 },
        { node: node("notes/d.org::0", "Delta"), distance: 2 },
        { node: node("notes/e.org::0", "Epsilon"), distance: 2 },
      ]),
    );

    expect(nearestOf(rings, 1).hidden).toBe(3);
  });
});
