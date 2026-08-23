import { describe, expect, it } from "vitest";

import {
  boundedRelated,
  rankedTotal,
  relatedGroups,
  relatedRowTotal,
  type RelatedGroup,
} from "./related.js";
import type {
  AnchorRecord,
  BridgeEvidenceRecord,
  ExplorationEntry,
  ExploreResult,
} from "../api/types.js";

function anchor(
  key: string,
  title: string,
  explicitId: string | null = null,
): AnchorRecord {
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

function via(key: string, title: string): BridgeEvidenceRecord {
  return { node_key: key, explicit_id: null, title };
}

function bridge(
  candidate: AnchorRecord,
  viaNotes: BridgeEvidenceRecord[],
  references: string[] = [],
): ExplorationEntry {
  return {
    kind: "anchor",
    anchor: candidate,
    explanation: {
      kind: "bridge-candidate",
      references,
      via_notes: viaNotes,
    },
  };
}

function bridges(entries: ExplorationEntry[]): ExploreResult {
  return {
    lens: "bridges",
    sections: [{ kind: "bridge-candidates", entries }],
  };
}

function titlesOf(groups: readonly RelatedGroup[]): string[][] {
  return groups.map((group) => group.rows.map((row) => row.title));
}

const NOTHING_LISTED: ReadonlySet<string> = new Set();

describe("relatedGroups", () => {
  it("keeps the lens's order inside a group", () => {
    const groups = relatedGroups(
      bridges([
        bridge(anchor("notes/wide.org::0", "Wide"), [
          via("notes/c1.org::0", "Measure"),
          via("notes/c2.org::0", "Entropy"),
        ]),
        bridge(anchor("notes/narrow.org::0", "Narrow"), [
          via("notes/c1.org::0", "Measure"),
        ]),
      ]),
      NOTHING_LISTED,
    );

    expect(titlesOf(groups)).toEqual([["Wide", "Narrow"]]);
    expect(groups[0]!.rows.map((row) => row.connectors)).toEqual([2, 1]);
  });

  it("names each connector once, in the order it first appears", () => {
    const groups = relatedGroups(
      bridges([
        bridge(anchor("notes/a.org::0", "Alpha"), [via("notes/c1.org::0", "Measure")]),
        bridge(anchor("notes/b.org::0", "Beta"), [via("notes/c2.org::0", "Entropy")]),
        bridge(anchor("notes/c.org::0", "Gamma"), [via("notes/c1.org::0", "Measure")]),
      ]),
      NOTHING_LISTED,
    );

    expect(groups.map((group) => group.connector)).toEqual(["Measure", "Entropy"]);
    expect(titlesOf(groups)).toEqual([["Alpha", "Gamma"], ["Beta"]]);
  });

  it("keeps two connectors that share a title apart", () => {
    const groups = relatedGroups(
      bridges([
        bridge(anchor("notes/a.org::0", "Alpha"), [via("notes/c1.org::0", "Measure")]),
        bridge(anchor("notes/b.org::0", "Beta"), [via("notes/c2.org::0", "Measure")]),
      ]),
      NOTHING_LISTED,
    );

    expect(groups.map((group) => group.connector)).toEqual(["Measure", "Measure"]);
    expect(titlesOf(groups)).toEqual([["Alpha"], ["Beta"]]);
  });

  it("drops a note the directed inventory already lists", () => {
    const groups = relatedGroups(
      bridges([
        bridge(anchor("notes/a.org::0", "Alpha"), [via("notes/c1.org::0", "Measure")]),
        bridge(anchor("notes/b.org::0", "Beta"), [via("notes/c1.org::0", "Measure")]),
      ]),
      new Set(["notes/a.org::0"]),
    );

    expect(titlesOf(groups)).toEqual([["Beta"]]);
  });

  it("leaves a connector unnamed when every row under it is already listed", () => {
    const groups = relatedGroups(
      bridges([
        bridge(anchor("notes/a.org::0", "Alpha"), [via("notes/c1.org::0", "Measure")]),
      ]),
      new Set(["notes/a.org::0"]),
    );

    expect(groups).toEqual([]);
  });

  it("reads only bridge candidates, whatever else the answer carries", () => {
    const groups = relatedGroups(
      {
        lens: "bridges",
        sections: [
          {
            kind: "forward-links",
            entries: [
              bridge(anchor("notes/wrong-section.org::0", "Wrong section"), [
                via("notes/c1.org::0", "Measure"),
              ]),
            ],
          },
          {
            kind: "bridge-candidates",
            entries: [
              {
                kind: "anchor",
                anchor: anchor("notes/wrong-reason.org::0", "Wrong reason"),
                explanation: { kind: "shared-reference", reference: "@key" },
              },
              bridge(anchor("notes/right.org::0", "Right"), [
                via("notes/c1.org::0", "Measure"),
              ]),
            ],
          },
        ],
      },
      NOTHING_LISTED,
    );

    expect(titlesOf(groups)).toEqual([["Right"]]);
  });

  it("drops a candidate reached through nothing", () => {
    const groups = relatedGroups(
      bridges([bridge(anchor("notes/a.org::0", "Alpha"), [])]),
      NOTHING_LISTED,
    );

    expect(groups).toEqual([]);
  });

  it("prefers an id target when the candidate carries an explicit id", () => {
    const groups = relatedGroups(
      bridges([
        bridge(anchor("notes/a.org::0", "Alpha", "uuid-1"), [
          via("notes/c1.org::0", "Measure"),
        ]),
      ]),
      NOTHING_LISTED,
    );

    expect(groups[0]!.rows[0]!.target).toEqual({
      id: "uuid-1",
      target: "id:uuid-1",
    });
  });
});

describe("rankedTotal", () => {
  it("counts what the lens ranked, not what survives the exclusion", () => {
    const answer = bridges([
      bridge(anchor("notes/a.org::0", "Alpha"), [via("notes/c1.org::0", "Measure")]),
      bridge(anchor("notes/b.org::0", "Beta"), [via("notes/c1.org::0", "Measure")]),
    ]);

    expect(rankedTotal(answer)).toBe(2);
    expect(relatedRowTotal(relatedGroups(answer, new Set(["notes/a.org::0"])))).toBe(1);
  });

  it("counts no entry the lens did not admit as a bridge", () => {
    expect(
      rankedTotal({
        lens: "bridges",
        sections: [
          {
            kind: "bridge-candidates",
            entries: [bridge(anchor("notes/a.org::0", "Alpha"), [])],
          },
        ],
      }),
    ).toBe(0);
  });
});

describe("relatedRowTotal", () => {
  it("counts the rows across every group, not the groups", () => {
    const groups = relatedGroups(
      bridges([
        bridge(anchor("notes/a.org::0", "Alpha"), [via("notes/c1.org::0", "Measure")]),
        bridge(anchor("notes/b.org::0", "Beta"), [via("notes/c2.org::0", "Entropy")]),
        bridge(anchor("notes/c.org::0", "Gamma"), [via("notes/c2.org::0", "Entropy")]),
      ]),
      NOTHING_LISTED,
    );

    expect(relatedRowTotal(groups)).toBe(3);
  });
});

describe("boundedRelated", () => {
  const GROUPS: RelatedGroup[] = [
    {
      connector: "Measure",
      rows: [
        { key: "a", target: { id: null, target: "a" }, title: "Alpha", connectors: 3 },
        { key: "b", target: { id: null, target: "b" }, title: "Beta", connectors: 1 },
      ],
    },
    {
      connector: "Entropy",
      rows: [
        { key: "c", target: { id: null, target: "c" }, title: "Gamma", connectors: 2 },
      ],
    },
  ];

  it("cuts on the ranking rather than on the group boundary", () => {
    expect(titlesOf(boundedRelated(GROUPS, 2))).toEqual([["Alpha"], ["Gamma"]]);
    expect(titlesOf(boundedRelated(GROUPS, 1))).toEqual([["Alpha"]]);
  });

  it("keeps the group order and the order inside a group", () => {
    const connectors = boundedRelated(GROUPS, 3).map((group) => group.connector);
    expect(connectors).toEqual(["Measure", "Entropy"]);
    expect(titlesOf(boundedRelated(GROUPS, 3))).toEqual([
      ["Alpha", "Beta"],
      ["Gamma"],
    ]);
  });

  it("leaves the groups whole when the bound reaches past them", () => {
    expect(boundedRelated(GROUPS, 8)).toEqual(GROUPS);
  });
});
