import { describe, expect, it } from "vitest";

import {
  RELATION_PREVIEW_CHARS,
  relationRows,
  shownInDirection,
} from "./relations.js";
import type { Inline } from "../org/types.js";
import type {
  BacklinkRecord,
  ForwardLinkRecord,
  NodeRecord,
  NoteContext,
} from "../api/types.js";

function characters(prose: readonly Inline[]): string {
  return prose
    .map((node) => {
      switch (node.type) {
        case "text":
          return node.value;
        case "bold":
        case "italic":
          return characters(node.children);
        case "verbatim":
          return node.value;
        default:
          return "";
      }
    })
    .join("");
}

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
    backlink_note_total: backlinks.length,
    forward_link_note_total: forward_links.length,
  };
}

describe("relationRows", () => {
  it("lists forward links in server order, marked outbound", () => {
    const rows = relationRows(
      context(
        [
          forward(node("notes/a.org::0", "Alpha")),
          forward(node("notes/b.org::0", "Beta")),
        ],
        [],
      ),
    );

    expect(rows.map((row) => row.title)).toEqual(["Alpha", "Beta"]);
    expect(rows.map((row) => row.direction)).toEqual(["out", "out"]);
  });

  it("carries no preview on a forward-only row", () => {
    const rows = relationRows(
      context([forward(node("notes/a.org::0", "Alpha"), "see Alpha")], []),
    );

    expect(rows[0]!.preview).toEqual([]);
  });

  it("quotes the source note's line on an inbound row", () => {
    const rows = relationRows(
      context([], [backward(node("notes/x.org::0", "Ex"), "  cites Self  ")]),
    );

    expect(rows[0]!.direction).toBe("in");
    // The preview is trimmed.
    expect(rows[0]!.preview).toEqual([{ type: "text", value: "cites Self" }]);
  });

  it("parses raw Org markup in the preview into renderable prose", () => {
    const rows = relationRows(
      context(
        [],
        [
          backward(
            node("notes/x.org::0", "Ex"),
            "as [[id:xyz][the lemma]] shows, \\(x = 0\\) under /mild/ conditions",
          ),
        ],
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

  it("lists a reciprocal note once, marked both ways, where it first stood", () => {
    const rows = relationRows(
      context(
        [
          forward(node("notes/a.org::0", "Alpha"), "cites Alpha"),
          forward(node("notes/b.org::0", "Beta")),
        ],
        [backward(node("notes/a.org::0", "Alpha"), "cites Self back")],
      ),
    );

    expect(rows.map((row) => row.title)).toEqual(["Alpha", "Beta"]);
    expect(rows.map((row) => row.direction)).toEqual(["both", "out"]);
    expect(rows[0]!.preview).toEqual([{ type: "text", value: "cites Self back" }]);
  });

  it("deduplicates repeated destinations, keeping the first", () => {
    const rows = relationRows(
      context(
        [
          forward(node("notes/a.org::0", "Alpha"), "first"),
          forward(node("notes/a.org::0", "Alpha"), "second"),
        ],
        [],
      ),
    );

    expect(rows).toHaveLength(1);
  });

  it("deduplicates repeated sources, keeping the first line", () => {
    const rows = relationRows(
      context(
        [],
        [
          backward(node("notes/x.org::0", "Ex"), "first"),
          backward(node("notes/x.org::0", "Ex"), "second"),
        ],
      ),
    );

    expect(rows).toHaveLength(1);
    expect(rows[0]!.preview).toEqual([{ type: "text", value: "first" }]);
  });

  it("bounds a preview by characters, not by the clip that paints it", () => {
    const paragraph = "quantum ".repeat(RELATION_PREVIEW_CHARS);
    const rows = relationRows(
      context([], [backward(node("notes/x.org::0", "Ex"), paragraph)]),
    );

    const text = characters(rows[0]!.preview);
    expect(text.length).toBeLessThanOrEqual(RELATION_PREVIEW_CHARS + 1);
    expect(text.endsWith("…")).toBe(true);
  });

  it("leaves a preview inside the bound whole and unmarked", () => {
    const rows = relationRows(
      context([], [backward(node("notes/x.org::0", "Ex"), "cites Self once")]),
    );

    expect(rows[0]!.preview).toEqual([
      { type: "text", value: "cites Self once" },
    ]);
  });

  it("prefers an id target when the note carries an explicit id", () => {
    const withId = relationRows(
      context([forward(node("notes/a.org::0", "Alpha", "uuid-1"))], []),
    );
    expect(withId[0]!.target).toEqual({ id: "uuid-1", target: "id:uuid-1" });

    const withoutId = relationRows(
      context([forward(node("notes/a.org::0", "Alpha"))], []),
    );
    expect(withoutId[0]!.target).toEqual({
      id: null,
      target: "notes/a.org::0",
    });
  });
});

describe("shownInDirection", () => {
  it("counts a both-ways row in each direction", () => {
    const rows = relationRows(
      context(
        [forward(node("notes/a.org::0", "Alpha"))],
        [
          backward(node("notes/a.org::0", "Alpha")),
          backward(node("notes/x.org::0", "Ex")),
        ],
      ),
    );

    expect(shownInDirection(rows, "out")).toBe(1);
    expect(shownInDirection(rows, "in")).toBe(2);
  });
});
