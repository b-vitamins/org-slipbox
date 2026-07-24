import { describe, expect, it } from "vitest";

import { parseInline } from "./parse-inline.js";
import { parseOrg } from "./parse.js";
import { documentPreview, inlinePreview } from "./prose.js";
import type { Inline } from "./types.js";

/** The text a run of prose reads as, so a case can assert on words alone. */
function words(nodes: readonly Inline[]): string {
  return nodes
    .map((node) => {
      switch (node.type) {
        case "text":
          return node.value;
        case "bold":
        case "italic":
          return words(node.children);
        case "verbatim":
          return node.value;
        case "math":
          return `⟨${node.tex}⟩`;
        default:
          return "";
      }
    })
    .join("");
}

describe("inlinePreview", () => {
  it("keeps math as math rather than flattening it to its source", () => {
    const prose = inlinePreview(parseInline("density \\\\(p_z(\\\\mathbf{z})\\\\) here"), 200);
    expect(prose).toEqual([
      { type: "text", value: "density " },
      { type: "math", tex: "p_z(\\mathbf{z})" },
      { type: "text", value: " here" },
    ]);
  });

  it("unwraps a link to its label, so a preview carries no second link", () => {
    const prose = inlinePreview(parseInline("as [[id:xyz][the lemma]] shows"), 200);
    expect(prose).toEqual([{ type: "text", value: "as the lemma shows" }]);
  });

  it("keeps emphasis and verbatim as themselves", () => {
    const prose = inlinePreview(parseInline("a /clear/ =x= case"), 200);
    expect(prose).toEqual([
      { type: "text", value: "a " },
      { type: "italic", children: [{ type: "text", value: "clear" }] },
      { type: "text", value: " " },
      { type: "verbatim", value: "x" },
      { type: "text", value: " case" },
    ]);
  });

  it("collapses whitespace and trims the ends", () => {
    expect(inlinePreview(parseInline("  see   Alpha  "), 200)).toEqual([
      { type: "text", value: "see Alpha" },
    ]);
  });

  it("cuts running text on a word boundary and marks the elision", () => {
    const prose = inlinePreview(parseInline("alpha beta gamma delta"), 12);
    expect(words(prose)).toBe("alpha beta…");
  });

  it("drops a formula that does not fit rather than cutting it", () => {
    const prose = inlinePreview(parseInline("the bound \\\\(\\\\sum_n x_n\\\\)"), 12);
    expect(words(prose)).toBe("the bound…");
    expect(prose.some((node) => node.type === "math")).toBe(false);
  });

  it("keeps a formula that fits within the budget", () => {
    const prose = inlinePreview(parseInline("bound \\\\(x\\\\) holds"), 200);
    expect(words(prose)).toBe("bound ⟨x⟩ holds");
  });

  it("returns nothing for prose that is only whitespace", () => {
    expect(inlinePreview(parseInline("   "), 200)).toEqual([]);
  });

  it("keeps an over-wide formula only when nothing else would show", () => {
    const wide = "\\\\sum_{n=1}^{N} \\\\alpha_n \\\\beta_n \\\\gamma_n \\\\delta_n";

    // Nothing before it, so the over-wide atom is kept whole.
    const alone = inlinePreview(parseInline(`\\\\(${wide}\\\\)`), 20);
    expect(alone.some((node) => node.type === "math")).toBe(true);

    // Words before it, at the top level and nested one and two deep.
    for (const source of [
      `the bound \\\\(${wide}\\\\)`,
      `the bound *\\\\(${wide}\\\\)*`,
      `the bound */\\\\(${wide}\\\\)/*`,
    ]) {
      const prose = inlinePreview(parseInline(source), 20);
      expect(words(prose)).toBe("the bound…");
    }
  });
});

describe("documentPreview", () => {
  it("joins blocks into one line", () => {
    const prose = documentPreview(parseOrg("* Title\n\nFirst line.\n\nSecond line."), 200);
    expect(words(prose)).toBe("Title First line. Second line.");
  });

  it("sets a display-math block inline among the words", () => {
    const doc = parseOrg("Given\n\n\\\\[\nJ = \\\\sum_n r_n\n\\\\]\n\nit follows.");
    const prose = documentPreview(doc, 200);
    expect(words(prose)).toBe("Given ⟨J = \\sum_n r_n⟩ it follows.");
    expect(prose.filter((node) => node.type === "math")).toHaveLength(1);
  });

  it("flattens a table's cells to their prose across header and body", () => {
    const doc = parseOrg("| quantity | value |\n|---+---|\n| \\\\(d\\\\) | 0 |");
    expect(words(documentPreview(doc, 200))).toBe("quantity value ⟨d⟩ 0");
  });

  it("reads a quoted passage's words as part of the preview", () => {
    const doc = parseOrg("Consider\n\n#+begin_quote\nthe territory.\n#+end_quote");
    expect(words(documentPreview(doc, 200))).toBe("Consider the territory.");
  });

  it("returns the whole prose untouched when within the limit", () => {
    expect(documentPreview(parseOrg("short note"), 200)).toEqual([
      { type: "text", value: "short note" },
    ]);
  });

  it("bounds a long body on a word boundary", () => {
    const prose = documentPreview(parseOrg("alpha beta gamma delta epsilon"), 12);
    expect(words(prose)).toBe("alpha beta…");
  });
});
