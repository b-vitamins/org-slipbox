import { describe, expect, it } from "vitest";

import { MAX_EMPHASIS_DEPTH, parseInline } from "./parse-inline.js";
import { parseOrg } from "./parse.js";
import type { Inline } from "./types.js";

function asLink(node: Inline): Extract<Inline, { type: "link" }> {
  if (node.type !== "link") {
    throw new Error(`expected a link, got ${node.type}`);
  }
  return node;
}

/** Nesting depth of an inline run, the outermost span counting as one. */
function nestingDepth(nodes: readonly Inline[]): number {
  let deepest = 0;
  for (const node of nodes) {
    if (node.type === "link") {
      deepest = Math.max(deepest, nestingDepth(node.label) + 1);
    } else if ("children" in node) {
      deepest = Math.max(deepest, nestingDepth(node.children) + 1);
    }
  }
  return deepest;
}

function flatten(nodes: readonly Inline[]): string {
  return nodes
    .map((node) => {
      switch (node.type) {
        case "text":
        case "verbatim":
          return node.value;
        case "math":
          return node.tex;
        case "link":
          return flatten(node.label);
        default:
          return flatten(node.children);
      }
    })
    .join("");
}

describe("parseInline", () => {
  it("keeps plain text as a single text node", () => {
    expect(parseInline("just words")).toEqual([{ type: "text", value: "just words" }]);
  });

  it("parses an id link into id and inline label", () => {
    const [node] = parseInline("see [[id:35918484-b051][K-means algorithm]] here");
    const link = asLink(parseInline("[[id:abc-123][K-means algorithm]]")[0]!);
    expect(link.id).toBe("abc-123");
    expect(link.target).toBe("id:abc-123");
    expect(link.label).toEqual([{ type: "text", value: "K-means algorithm" }]);
    expect(node).toEqual({ type: "text", value: "see " });
  });

  it("treats a bare link target as its own label", () => {
    const link = asLink(parseInline("[[id:xyz]]")[0]!);
    expect(link.id).toBe("xyz");
    expect(link.label).toEqual([{ type: "text", value: "id:xyz" }]);
  });

  it("renders italic and bold with Org border rules", () => {
    expect(parseInline("the /distortion measure/ here")).toEqual([
      { type: "text", value: "the " },
      { type: "italic", children: [{ type: "text", value: "distortion measure" }] },
      { type: "text", value: " here" },
    ]);
    expect(parseInline("a *bold* word")).toEqual([
      { type: "text", value: "a " },
      { type: "bold", children: [{ type: "text", value: "bold" }] },
      { type: "text", value: " word" },
    ]);
  });

  it("does not treat a slash between words as italic", () => {
    expect(parseInline("a 1/K encoding")).toEqual([{ type: "text", value: "a 1/K encoding" }]);
    expect(parseInline("the E/M steps")).toEqual([{ type: "text", value: "the E/M steps" }]);
  });

  it("keeps verbatim literal without nested markup", () => {
    expect(parseInline("set =r_{nk}= here")).toEqual([
      { type: "text", value: "set " },
      { type: "verbatim", value: "r_{nk}" },
      { type: "text", value: " here" },
    ]);
  });

  it("parses inline math and collapses doubled backslashes", () => {
    expect(parseInline("in \\\\(\\\\mathbb{R}^D\\\\) space")).toEqual([
      { type: "text", value: "in " },
      { type: "math", tex: "\\mathbb{R}^D" },
      { type: "text", value: " space" },
    ]);
  });

  it("also accepts single-backslash math delimiters", () => {
    expect(parseInline("\\(x^2\\)")).toEqual([{ type: "math", tex: "x^2" }]);
  });

  it("preserves a single-backslash span's row break instead of collapsing it", () => {
    // `\(...\)` is single-encoded, so `\\` is a row break, not a pair.
    expect(parseInline("\\(a\\\\b\\)")).toEqual([{ type: "math", tex: "a\\\\b" }]);
  });

  it("leaves empty math delimiters as the literal text they are", () => {
    expect(parseInline("an \\(\\) empty span")).toEqual([
      { type: "text", value: "an \\(\\) empty span" },
    ]);
  });

  it("does not read a slash inside math as emphasis", () => {
    const nodes = parseInline("\\\\(a/b\\\\) and /real/");
    expect(nodes[0]).toEqual({ type: "math", tex: "a/b" });
    expect(nodes[2]).toEqual({
      type: "italic",
      children: [{ type: "text", value: "real" }],
    });
  });

  it("nests emphasis as deep as prose goes", () => {
    expect(parseInline("*bold with /italic/ inside*")).toEqual([
      {
        type: "bold",
        children: [
          { type: "text", value: "bold with " },
          { type: "italic", children: [{ type: "text", value: "italic" }] },
          { type: "text", value: " inside" },
        ],
      },
    ]);
  });

  it("stops descending into emphasis rather than exhausting the stack", () => {
    const nested = (borders: number): number => {
      const pathological = `${"/".repeat(borders)}x${"/".repeat(borders)}`;
      const nodes = parseInline(pathological);
      expect(flatten(nodes)).toContain("x");
      return nestingDepth(nodes);
    };
    // Depth stops growing with the input: 40x the borders nests no deeper.
    expect(nested(5000)).toBe(nested(125));
    expect(nested(5000)).toBeLessThanOrEqual(MAX_EMPHASIS_DEPTH);
  });

  it("scans a run of unpartnered markup without rereading it per marker", () => {
    // A timing budget, not a behaviour: a scan that rereads to the end of the
    // run once per unpartnered opener is quadratic and blows past 2s here.
    const runs = [
      "a *b ".repeat(40_000),
      "cost \\(x ".repeat(25_000),
      "a /b/ *c* =d= \\(x\\) [[id:1][l]] ".repeat(6_000),
    ];
    const started = performance.now();
    for (const run of runs) {
      expect(flatten(parseInline(run)).length).toBeGreaterThan(0);
    }
    expect(performance.now() - started).toBeLessThan(2_000);
  });
});

describe("parseOrg", () => {
  it("drops the properties drawer and keyword frontmatter", () => {
    const source = [
      ":PROPERTIES:",
      ":ID:       92128efe-bd70-427d-8ce5-8c9a583662b7",
      ":END:",
      "#+TITLE: K-means objective",
      "#+DATE: 2026-06-21",
      "#+FILETAGS: :ml:",
      "",
      "Body text.",
    ].join("\n");
    expect(parseOrg(source).blocks).toEqual([
      { type: "paragraph", children: [{ type: "text", value: "Body text." }] },
    ]);
  });

  it("joins wrapped paragraph lines and splits on blank lines", () => {
    const doc = parseOrg("one\ntwo\n\nthree");
    expect(doc.blocks).toEqual([
      { type: "paragraph", children: [{ type: "text", value: "one two" }] },
      { type: "paragraph", children: [{ type: "text", value: "three" }] },
    ]);
  });

  it("parses headings at their level", () => {
    const doc = parseOrg("* One\n** Two");
    expect(doc.blocks).toEqual([
      { type: "heading", level: 1, children: [{ type: "text", value: "One" }] },
      { type: "heading", level: 2, children: [{ type: "text", value: "Two" }] },
    ]);
  });

  it("parses unordered and ordered lists", () => {
    const unordered = parseOrg("- a\n- b");
    expect(unordered.blocks[0]).toEqual({
      type: "list",
      ordered: false,
      start: 1,
      items: [
        { children: [{ type: "text", value: "a" }], blocks: [] },
        { children: [{ type: "text", value: "b" }], blocks: [] },
      ],
    });
    const ordered = parseOrg("1. first\n2. second");
    expect(ordered.blocks[0]).toMatchObject({ type: "list", ordered: true });
  });

  it("keeps a blank-separated list as one list so numbering does not reset", () => {
    const doc = parseOrg("1. first\n\n2. second\n\n3. third");
    expect(doc.blocks).toHaveLength(1);
    expect(doc.blocks[0]).toEqual({
      type: "list",
      ordered: true,
      start: 1,
      items: [
        { children: [{ type: "text", value: "first" }], blocks: [] },
        { children: [{ type: "text", value: "second" }], blocks: [] },
        { children: [{ type: "text", value: "third" }], blocks: [] },
      ],
    });
  });

  it("joins an item's indented continuation lines into the item", () => {
    const doc = parseOrg(
      [
        "- A type is [[id:aaa][a predicate]], built with",
        "  =make-type=.",
        "- A superclass call is [[id:ccc][a =super= continuation handed to a chained",
        "  handler]].",
      ].join("\n"),
    );
    expect(doc.blocks).toHaveLength(1);
    const list = doc.blocks[0]!;
    if (list.type !== "list") {
      throw new Error("expected a list");
    }
    expect(list.items).toHaveLength(2);
    expect(list.items[0]!.children).toEqual([
      { type: "text", value: "A type is " },
      {
        type: "link",
        target: "id:aaa",
        id: "aaa",
        label: [{ type: "text", value: "a predicate" }],
      },
      { type: "text", value: ", built with " },
      { type: "verbatim", value: "make-type" },
      { type: "text", value: "." },
    ]);
    expect(asLink(list.items[1]!.children[1]!).label).toEqual([
      { type: "text", value: "a " },
      { type: "verbatim", value: "super" },
      { type: "text", value: " continuation handed to a chained handler" },
    ]);
  });

  it("nests a list indented under an item inside that item", () => {
    const doc = parseOrg("- outer one\n  - inner a\n  - inner b\n- outer two");
    expect(doc.blocks).toHaveLength(1);
    expect(doc.blocks[0]).toEqual({
      type: "list",
      ordered: false,
      start: 1,
      items: [
        {
          children: [{ type: "text", value: "outer one" }],
          blocks: [
            {
              type: "list",
              ordered: false,
              start: 1,
              items: [
                { children: [{ type: "text", value: "inner a" }], blocks: [] },
                { children: [{ type: "text", value: "inner b" }], blocks: [] },
              ],
            },
          ],
        },
        { children: [{ type: "text", value: "outer two" }], blocks: [] },
      ],
    });
  });

  it("keeps a step's own equation inside that step", () => {
    const doc = parseOrg(
      ["1. first", "", "2. second:", "   \\[", "   a = b", "   \\]", "", "3. third"].join("\n"),
    );
    expect(doc.blocks).toHaveLength(1);
    const list = doc.blocks[0]!;
    if (list.type !== "list") {
      throw new Error("expected a list");
    }
    expect(list).toMatchObject({ ordered: true, start: 1 });
    expect(list.items).toHaveLength(3);
    expect(list.items[1]!.blocks).toEqual([{ type: "math", tex: "a = b" }]);
    expect(list.items[0]!.blocks).toEqual([]);
    expect(list.items[2]!.children).toEqual([{ type: "text", value: "third" }]);
  });

  it("resumes an ordered list's numbering after an unindented interruption", () => {
    const doc = parseOrg("1. first\n2. second\n\nAn aside.\n\n3. third");
    expect(doc.blocks.map((block) => block.type)).toEqual(["list", "paragraph", "list"]);
    expect(doc.blocks[0]).toMatchObject({ ordered: true, start: 1 });
    expect(doc.blocks[2]).toMatchObject({ ordered: true, start: 3 });
  });

  it("starts a new list when the marker style changes", () => {
    const doc = parseOrg("- a\n1. b");
    expect(doc.blocks.map((block) => block.type)).toEqual(["list", "list"]);
    expect(doc.blocks[0]).toMatchObject({ ordered: false });
    expect(doc.blocks[1]).toMatchObject({ ordered: true });
  });

  it("ends a list at a blank line when the run does not resume with an item", () => {
    const doc = parseOrg("- a\n- b\n\nA following paragraph.");
    expect(doc.blocks).toHaveLength(2);
    expect(doc.blocks[0]).toMatchObject({ type: "list", ordered: false });
    expect(doc.blocks[1]).toEqual({
      type: "paragraph",
      children: [{ type: "text", value: "A following paragraph." }],
    });
  });

  it("consumes a drawer under a heading rather than showing its lines", () => {
    const doc = parseOrg("* A heading\n:PROPERTIES:\n:ID: deadbeef\n:END:\n\nBody.");
    expect(doc.blocks).toEqual([
      { type: "heading", level: 1, children: [{ type: "text", value: "A heading" }] },
      { type: "paragraph", children: [{ type: "text", value: "Body." }] },
    ]);
  });

  it("reads a note whose lines end with a carriage return", () => {
    const doc = parseOrg("- one\r\n- two\r\n\r\nA paragraph.\r\n");
    expect(doc.blocks).toHaveLength(2);
    expect(doc.blocks[0]).toMatchObject({ type: "list", ordered: false });
    expect(doc.blocks[1]).toEqual({
      type: "paragraph",
      children: [{ type: "text", value: "A paragraph." }],
    });
  });

  it("captures a source block with its language, verbatim", () => {
    const doc = parseOrg("#+begin_src python\nx = 1\n\ny = 2\n#+end_src");
    expect(doc.blocks[0]).toEqual({
      type: "src",
      lang: "python",
      code: "x = 1\n\ny = 2",
    });
  });

  it("ends an unclosed fence at the next heading rather than at the note's end", () => {
    const doc = parseOrg("#+begin_src scheme\n(define x 1)\n\n* Next section\n\nBody.");
    expect(doc.blocks.map((block) => block.type)).toEqual(["src", "heading", "paragraph"]);
    expect(doc.blocks[0]).toEqual({ type: "src", lang: "scheme", code: "(define x 1)\n" });
  });

  it("keeps a closed fence's body verbatim however the lines read", () => {
    const doc = parseOrg(
      "#+begin_src org\n* A heading\n#+begin_example\nnested\n#+end_src\n\nAfter.",
    );
    expect(doc.blocks).toEqual([
      {
        type: "src",
        lang: "org",
        code: "* A heading\n#+begin_example\nnested",
      },
      { type: "paragraph", children: [{ type: "text", value: "After." }] },
    ]);
  });

  it("does not let an unclosed fence claim the next fence's close", () => {
    const doc = parseOrg("#+begin_src py\na\n#+begin_src py\nb\n#+end_src");
    expect(doc.blocks).toEqual([
      { type: "src", lang: "py", code: "a" },
      { type: "src", lang: "py", code: "b" },
    ]);
  });

  it("reads an unclosed drawer as prose rather than losing the note to it", () => {
    const doc = parseOrg(":LOGBOOK:\nclock entry\n\nBody text.");
    expect(doc.blocks).toEqual([
      {
        type: "paragraph",
        children: [{ type: "text", value: ":LOGBOOK: clock entry" }],
      },
      { type: "paragraph", children: [{ type: "text", value: "Body text." }] },
    ]);
  });

  it("reads a whole formula written on one line as display math", () => {
    expect(parseOrg("\\[ J = \\sum_n r_n \\]").blocks).toEqual([
      { type: "math", tex: "J = \\sum_n r_n" },
    ]);
    expect(parseOrg("\\\\[ J = \\\\sum_n r_n \\\\]").blocks).toEqual([
      { type: "math", tex: "J = \\sum_n r_n" },
    ]);
  });

  it("leaves display math that prose continues past inside the paragraph", () => {
    expect(parseOrg("cost \\[x\\] and more").blocks).toEqual([
      {
        type: "paragraph",
        children: [
          { type: "text", value: "cost " },
          { type: "math", tex: "x" },
          { type: "text", value: " and more" },
        ],
      },
    ]);
  });

  it("takes a closing delimiter at the end of a body line as the close", () => {
    const doc = parseOrg("\\[\nJ = \\sum_n x_n \\]\n\nAfter.");
    expect(doc.blocks).toEqual([
      { type: "math", tex: "J = \\sum_n x_n" },
      { type: "paragraph", children: [{ type: "text", value: "After." }] },
    ]);
  });

  it("captures a display-math block and normalizes its tex", () => {
    const doc = parseOrg("\\\\[\nJ = \\\\sum_n r_n\n\\\\]");
    expect(doc.blocks[0]).toEqual({ type: "math", tex: "J = \\sum_n r_n" });
  });

  it("passes single-backslash display math through untouched", () => {
    // Collapsing a single-encoded `\\` would fuse the row break into the next
    // command: `\\\xi` -> `\\xi`.
    const doc = parseOrg("\\[\n\\begin{pmatrix}\\eta\\\\\\xi\\end{pmatrix}\n\\]");
    expect(doc.blocks[0]).toEqual({
      type: "math",
      tex: "\\begin{pmatrix}\\eta\\\\\\xi\\end{pmatrix}",
    });
  });

  it("hands a LaTeX display environment to the math renderer whole", () => {
    // KaTeX reads `\begin{align*}...\end{align*}` itself.
    const doc = parseOrg("\\begin{align*}\na &= b \\\\\n  &= c.\n\\end{align*}");
    expect(doc.blocks).toEqual([
      { type: "math", tex: "\\begin{align*}\na &= b \\\\\n  &= c.\n\\end{align*}" },
    ]);
  });

  it("ends unterminated math at the blank line rather than swallowing the note", () => {
    const doc = parseOrg("\\[\na = b\n\nTail prose.");
    expect(doc.blocks).toEqual([
      { type: "math", tex: "a = b" },
      { type: "paragraph", children: [{ type: "text", value: "Tail prose." }] },
    ]);
  });

  it("parses a table, marking the pre-rule row as header and dropping rule rows", () => {
    const doc = parseOrg("| a | b |\n|---+---|\n| 1 | 2 |");
    expect(doc.blocks[0]).toEqual({
      type: "table",
      rows: [
        {
          header: true,
          cells: [
            { children: [{ type: "text", value: "a" }] },
            { children: [{ type: "text", value: "b" }] },
          ],
        },
        {
          header: false,
          cells: [
            { children: [{ type: "text", value: "1" }] },
            { children: [{ type: "text", value: "2" }] },
          ],
        },
      ],
    });
  });

  it("finds the header of a boxed table between its first two rules", () => {
    // Org's header is the group of rows a rule separates from the body, which in
    // a table drawn with a rule above it is the second group, not the first.
    const doc = parseOrg("|---+---|\n| a | b |\n|---+---|\n| 1 | 2 |\n|---+---|");
    if (doc.blocks[0]?.type !== "table") {
      throw new Error("expected a table");
    }
    expect(doc.blocks[0].rows.map((row) => row.header)).toEqual([true, false]);
    expect(doc.blocks[0].rows[0]!.cells[0]!.children).toEqual([
      { type: "text", value: "a" },
    ]);
  });

  it("reads a table whose only rule is below its rows as all body", () => {
    const doc = parseOrg("| a | b |\n| 1 | 2 |\n|---+---|");
    if (doc.blocks[0]?.type !== "table") {
      throw new Error("expected a table");
    }
    expect(doc.blocks[0].rows.map((row) => row.header)).toEqual([false, false]);
  });

  it("keeps a row whose cells only look like a rule", () => {
    // Org's rule is `|---+---|`: dashes and separators only.
    const doc = parseOrg("| op | sign |\n|----+------|\n| - | + |");
    if (doc.blocks[0]?.type !== "table") {
      throw new Error("expected a table");
    }
    expect(doc.blocks[0].rows).toHaveLength(2);
    expect(doc.blocks[0].rows[1]!.cells.map((cell) => cell.children)).toEqual([
      [{ type: "text", value: "-" }],
      [{ type: "text", value: "+" }],
    ]);
  });

  it("parses inline math inside table cells rather than leaving raw TeX", () => {
    const doc = parseOrg("| \\\\(d \\\\approx 0\\\\) | text |\n| a | b |");
    if (doc.blocks[0]?.type !== "table") {
      throw new Error("expected a table");
    }
    expect(doc.blocks[0].rows[0]!.header).toBe(false);
    expect(doc.blocks[0].rows[0]!.cells[0]!.children).toEqual([
      { type: "math", tex: "d \\approx 0" },
    ]);
  });

  it("reads a quoted passage as a quote rather than as its directives", () => {
    const doc = parseOrg("#+begin_quote\nThe map is not the territory.\n#+end_quote");
    expect(doc.blocks).toEqual([
      {
        type: "quote",
        blocks: [
          {
            type: "paragraph",
            children: [{ type: "text", value: "The map is not the territory." }],
          },
        ],
      },
    ]);
  });

  it("drops a comment block, and keeps the body of a wrapper it has no block for", () => {
    expect(parseOrg("#+begin_comment\nnot for the reader\n#+end_comment\nVisible.").blocks)
      .toEqual([{ type: "paragraph", children: [{ type: "text", value: "Visible." }] }]);
    expect(parseOrg("#+begin_verse\nA line of verse.\n#+end_verse").blocks).toEqual([
      { type: "paragraph", children: [{ type: "text", value: "A line of verse." }] },
    ]);
  });

  it("leaves an unclosed wrapper's body as the prose it is", () => {
    expect(parseOrg("#+begin_quote\nAn unclosed quotation.").blocks).toEqual([
      { type: "paragraph", children: [{ type: "text", value: "An unclosed quotation." }] },
    ]);
  });

  it("drops a comment line without dividing the prose around it", () => {
    const doc = parseOrg("First half\n# a note to self\nsecond half.");
    expect(doc.blocks).toEqual([
      {
        type: "paragraph",
        children: [{ type: "text", value: "First half second half." }],
      },
    ]);
    // `#` comments a line only when it stands alone or a space follows it.
    expect(parseOrg("#hashtag prose").blocks).toEqual([
      { type: "paragraph", children: [{ type: "text", value: "#hashtag prose" }] },
    ]);
  });

  it("parses a realistic note end to end", () => {
    const source = [
      ":PROPERTIES:",
      ":ID:       92128efe",
      ":END:",
      "#+TITLE: K-means objective",
      "",
      "The K-means objective is the /distortion measure/",
      "",
      "\\\\[",
      "J = \\\\sum_{n=1}^{N} r_{nk}",
      "\\\\]",
      "",
      "approached via the [[id:35918484][K-means algorithm]].",
    ].join("\n");
    const doc = parseOrg(source);
    expect(doc.blocks).toHaveLength(3);
    expect(doc.blocks[0]!.type).toBe("paragraph");
    expect(doc.blocks[1]).toEqual({ type: "math", tex: "J = \\sum_{n=1}^{N} r_{nk}" });
    const last = doc.blocks[2]!;
    if (last.type !== "paragraph") {
      throw new Error("expected paragraph");
    }
    const link = last.children.find((n) => n.type === "link");
    expect(link).toEqual({
      type: "link",
      target: "id:35918484",
      id: "35918484",
      label: [{ type: "text", value: "K-means algorithm" }],
    });
  });
});
