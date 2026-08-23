import { describe, expect, it } from "vitest";

import { backwardRelations, forwardRelations } from "./relations.js";
import type {
  BacklinkRecord,
  ForwardLinkRecord,
  NodeRecord,
  NoteContext,
} from "../api/types.js";

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

function forward(dest: NodeRecord, preview = ""): ForwardLinkRecord {
  return {
    destination_note: dest,
    row: 1,
    col: 1,
    preview,
    explanation: { kind: "forward-link" },
  };
}

function backward(source: NodeRecord, preview = ""): BacklinkRecord {
  return {
    source_note: source,
    source_anchor: null,
    row: 1,
    col: 1,
    preview,
    explanation: { kind: "backlink" },
  };
}

function context(
  forward_links: ForwardLinkRecord[],
  backlinks: BacklinkRecord[],
): NoteContext {
  return {
    note: node("notes/self.org::0", "Self"),
    source: {
      file_path: "notes/self.org",
      start_line: 1,
      line_count: 1,
      total_lines: 1,
      content: "",
      truncated_before: false,
      truncated_after: false,
    },
    node_start_line: 1,
    node_line_count: 1,
    place: { ordinal: 1, total: 1 },
    backlinks,
    forward_links,
    // Neither projection reads a total; the footer above them does.
    backlink_note_total: backlinks.length,
    forward_link_note_total: forward_links.length,
  };
}

describe("forwardRelations", () => {
  it("projects each distinct destination in server order", () => {
    const rows = forwardRelations(
      context(
        [
          forward(node("notes/a.org::0", "Alpha"), "  see Alpha  "),
          forward(node("notes/b.org::0", "Beta")),
        ],
        [],
      ),
    );

    expect(rows.map((row) => row.title)).toEqual(["Alpha", "Beta"]);
    // The preview is trimmed.
    expect(rows[0]!.preview).toEqual([{ type: "text", value: "see Alpha" }]);
  });

  it("parses raw Org markup in the preview into renderable prose", () => {
    const rows = forwardRelations(
      context(
        [
          forward(
            node("notes/a.org::0", "Alpha"),
            "as [[id:xyz][the lemma]] shows, \\(x = 0\\) under /mild/ conditions",
          ),
        ],
        [],
      ),
    );

    // The math stays a math node, so the row typesets it rather than printing
    // its delimiters; the link contributes its label alone.
    expect(rows[0]!.preview).toEqual([
      { type: "text", value: "as the lemma shows, " },
      { type: "math", tex: "x = 0" },
      { type: "text", value: " under " },
      { type: "italic", children: [{ type: "text", value: "mild" }] },
      { type: "text", value: " conditions" },
    ]);
  });

  it("deduplicates repeated destinations, keeping the first", () => {
    const rows = forwardRelations(
      context(
        [
          forward(node("notes/a.org::0", "Alpha"), "first"),
          forward(node("notes/a.org::0", "Alpha"), "second"),
        ],
        [],
      ),
    );

    expect(rows).toHaveLength(1);
    expect(rows[0]!.preview).toEqual([{ type: "text", value: "first" }]);
  });

  it("prefers an id target when the note carries an explicit id", () => {
    const withId = forwardRelations(
      context([forward(node("notes/a.org::0", "Alpha", "uuid-1"))], []),
    );
    expect(withId[0]!.target).toEqual({ id: "uuid-1", target: "id:uuid-1" });

    const withoutId = forwardRelations(
      context([forward(node("notes/a.org::0", "Alpha"))], []),
    );
    expect(withoutId[0]!.target).toEqual({
      id: null,
      target: "notes/a.org::0",
    });
  });
});

describe("backwardRelations", () => {
  it("projects each distinct source in server order", () => {
    const rows = backwardRelations(
      context(
        [],
        [
          backward(node("notes/x.org::0", "Ex")),
          backward(node("notes/y.org::0", "Why")),
        ],
      ),
    );
    expect(rows.map((row) => row.title)).toEqual(["Ex", "Why"]);
  });

  it("deduplicates repeated sources", () => {
    const rows = backwardRelations(
      context(
        [],
        [
          backward(node("notes/x.org::0", "Ex")),
          backward(node("notes/x.org::0", "Ex")),
        ],
      ),
    );
    expect(rows).toHaveLength(1);
  });
});
