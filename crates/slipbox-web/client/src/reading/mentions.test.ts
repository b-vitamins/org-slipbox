import { describe, expect, it } from "vitest";

import type {
  AnchorRecord,
  UnlinkedReferenceRecord,
  UnlinkedReferencesResult,
} from "../api/types.js";
import type { Inline } from "../org/types.js";
import { mentionRows, type MentionRow } from "./mentions.js";
import { RELATION_PREVIEW_CHARS } from "./relations.js";

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

/**
 * One occurrence. The column defaults to where the text first stands, and the
 * line sits in `source` itself unless `under` names a node inside it.
 */
function mention(
  source: AnchorRecord,
  preview: string,
  matched: string,
  col = preview.indexOf(matched) + 1,
  row = 1,
  under: AnchorRecord = source,
): UnlinkedReferenceRecord {
  return {
    source_note: source,
    source_anchor: under,
    row,
    col,
    preview,
    matched_text: matched,
    explanation: { kind: "unlinked-reference", matched_text: matched },
  };
}

function scan(records: UnlinkedReferenceRecord[]): UnlinkedReferencesResult {
  return { unlinked_references: records };
}

const NOTHING_LISTED: ReadonlySet<string> = new Set<string>();

function text(nodes: readonly Inline[]): string {
  return nodes
    .map((node) => {
      switch (node.type) {
        case "text":
          return node.value;
        case "bold":
        case "italic":
          return text(node.children);
        case "verbatim":
          return node.value;
        case "math":
          return node.tex;
        case "link":
          return text(node.label);
      }
    })
    .join("");
}

/** The whole preview a row carries, marked runs included. */
function preview(row: MentionRow): string {
  return row.preview.map((run) => text(run.prose)).join("");
}

/** Only the runs the scan matched in. */
function marked(row: MentionRow): string[] {
  return row.preview.filter((run) => run.matched).map((run) => text(run.prose));
}

describe("mentionRows", () => {
  // The scan orders by indexed file path and then by position, which is the
  // order the rows stand in.
  it("keeps the order the scan sent", () => {
    const rows = mentionRows(
      scan([
        mention(anchor("notes/b.org::0", "Beta"), "Measure again", "Measure"),
        mention(anchor("notes/a.org::0", "Alpha"), "Measure once", "Measure"),
      ]),
      NOTHING_LISTED,
    );

    expect(rows.map((row) => row.title)).toEqual(["Beta", "Alpha"]);
  });

  // A reader acts on the note, so the offsets inside it are not each a row.
  it("gives one note one row, its first occurrence", () => {
    const source = anchor("notes/a.org::0", "Alpha");
    const rows = mentionRows(
      scan([
        mention(source, "Measure in the first place", "Measure"),
        mention(source, "Measure again lower down", "Measure", undefined, 9),
      ]),
      NOTHING_LISTED,
    );

    expect(rows).toHaveLength(1);
    expect(preview(rows[0]!)).toBe("Measure in the first place");
  });

  // The scan resolves the nearest node above the line, which may be a heading
  // carrying no id, while the inventory lists notes: read against the heading a
  // mention in a note that links here reads as news, which it is not.
  it("drops a listed note whose mention stands under a heading in it", () => {
    const note = anchor("notes/a.org::0", "Alpha");
    const rows = mentionRows(
      scan([
        mention(
          note,
          "Measure once",
          "Measure",
          undefined,
          7,
          anchor("notes/a.org::4", "A section of Alpha"),
        ),
        mention(anchor("notes/b.org::0", "Beta"), "Measure twice", "Measure"),
      ]),
      new Set(["notes/a.org::0"]),
    );

    expect(rows.map((row) => row.title)).toEqual(["Beta"]);
  });

  // Two headings of one note are two nodes of the index and one note to a
  // reader, so the note is named once, at its first occurrence.
  it("gives one note one row across the headings inside it", () => {
    const note = anchor("notes/a.org::0", "Alpha");
    const rows = mentionRows(
      scan([
        mention(
          note,
          "Measure in the first place",
          "Measure",
          undefined,
          7,
          anchor("notes/a.org::4", "First section"),
        ),
        mention(
          note,
          "Measure again lower down",
          "Measure",
          undefined,
          19,
          anchor("notes/a.org::17", "Second section"),
        ),
      ]),
      NOTHING_LISTED,
    );

    expect(rows).toHaveLength(1);
    expect(rows[0]!.title).toBe("Alpha");
    expect(rows[0]!.key).toBe("notes/a.org::0");
    expect(preview(rows[0]!)).toBe("Measure in the first place");
  });

  // A daemon older than the field answers no note, and where the line sits is
  // then all that is known of it.
  it("names the anchor where the payload carries no note", () => {
    const older: UnlinkedReferenceRecord = {
      ...mention(anchor("notes/a.org::4", "A section"), "Measure once", "Measure"),
      source_note: undefined,
    };
    const rows = mentionRows(scan([older]), NOTHING_LISTED);

    expect(rows.map((row) => row.title)).toEqual(["A section"]);
  });

  it("drops a node the directed inventory lists", () => {
    const rows = mentionRows(
      scan([
        mention(anchor("notes/a.org::0", "Alpha"), "Measure once", "Measure"),
        mention(anchor("notes/b.org::0", "Beta"), "Measure twice", "Measure"),
      ]),
      new Set(["notes/a.org::0"]),
    );

    expect(rows.map((row) => row.title)).toEqual(["Beta"]);
  });

  it("prefers an id target where the node has one", () => {
    const rows = mentionRows(
      scan([
        mention(anchor("notes/a.org::0", "Alpha", "uuid-1"), "Measure", "Measure"),
      ]),
      NOTHING_LISTED,
    );

    expect(rows[0]!.target).toEqual({ id: "uuid-1", target: "id:uuid-1" });
  });

  it("flags the matched run inside the preview rather than beside it", () => {
    const rows = mentionRows(
      scan([
        mention(
          anchor("notes/a.org::0", "Alpha"),
          "compared with Measure theory throughout",
          "Measure",
        ),
      ]),
      NOTHING_LISTED,
    );

    expect(marked(rows[0]!)).toEqual(["Measure"]);
    expect(preview(rows[0]!)).toBe("compared with Measure theory throughout");
  });

  // The scan passes over an occurrence an indexed link already covers, so the
  // one it reports may be the second of two in the same line.
  it("marks the occurrence the stated column names", () => {
    const rows = mentionRows(
      scan([
        mention(
          anchor("notes/a.org::0", "Alpha"),
          "Measure and Measure again",
          "Measure",
          13,
        ),
      ]),
      NOTHING_LISTED,
    );

    const [head, hit] = rows[0]!.preview;
    expect(text(head!.prose)).toBe("Measure and ");
    expect(hit!.matched).toBe(true);
  });

  // A row paints one clipped line, so a match past its end would be invisible.
  it("winds a long line forward to the match and marks the cut", () => {
    const rows = mentionRows(
      scan([
        mention(
          anchor("notes/a.org::0", "Alpha"),
          `${"padding ".repeat(20)}Measure at the end`,
          "Measure",
        ),
      ]),
      NOTHING_LISTED,
    );

    expect(preview(rows[0]!)).toMatch(/^…/);
    expect(marked(rows[0]!)).toEqual(["Measure"]);
  });

  // The clip is paint only: without a bound the whole line reaches the
  // accessible tree, as it would on a link row.
  it("carries no more of the line than a link row would", () => {
    const rows = mentionRows(
      scan([
        mention(
          anchor("notes/a.org::0", "Alpha"),
          `Measure ${"and more ".repeat(40)}`,
          "Measure",
        ),
      ]),
      NOTHING_LISTED,
    );

    // The two cut marks stand outside the bound.
    expect(preview(rows[0]!).length).toBeLessThanOrEqual(
      RELATION_PREVIEW_CHARS + 2,
    );
  });

  // The line is raw Org, and the match may stand inside a construct, so the
  // reduction runs over the whole window rather than around the match.
  it("reduces the markup the match stands in", () => {
    const rows = mentionRows(
      scan([
        mention(
          anchor("notes/a.org::0", "Alpha"),
          "see [[id:uuid-2][the Measure note]] for it",
          "Measure",
        ),
      ]),
      NOTHING_LISTED,
    );

    expect(preview(rows[0]!)).toBe("see the Measure note for it");
    expect(marked(rows[0]!)).toEqual(["Measure"]);
  });

  it("marks nothing when the line does not hold the stated text", () => {
    const rows = mentionRows(
      scan([
        mention(anchor("notes/a.org::0", "Alpha"), "nothing of the sort", "Measure", 1),
      ]),
      NOTHING_LISTED,
    );

    expect(marked(rows[0]!)).toEqual([]);
    expect(preview(rows[0]!)).toBe("nothing of the sort");
  });
});
