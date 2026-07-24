/*
 * Inline Org parsing in one left-to-right scan. Order is required: math spans and
 * links match whole first, so the emphasis pass cannot mistake a `/` or `=` inside
 * one for markup. Emphasis then follows Org's border rules, where a marker opens
 * only after a boundary character and closes only before one, keeping `1/K` text.
 */

import { mathDelimiterLength, normalizeTex } from "./tex.js";
import type { Inline } from "./types.js";

/**
 * How deep emphasis may nest before an inner span is kept as plain text. An
 * emphasis body is parsed as inline content and can open a span of its own, so
 * without a bound a text holding thousands of `/` borders recurses once per
 * border until the JS stack is exhausted.
 */
export const MAX_EMPHASIS_DEPTH = 32;

/** Characters permitted immediately before an opening emphasis marker. */
const PRE_MARKERS = new Set(["-", "(", "'", '"', "{"]);
/** Characters permitted immediately after a closing emphasis marker. */
const POST_MARKERS = new Set([
  "-", ".", ",", ":", "!", "?", ";", "'", '"', ")", "}", "[",
]);

/** The emphasis markers the corpus uses. `=` is literal (verbatim). */
const EMPHASIS: Record<string, "bold" | "italic" | "verbatim"> = {
  "*": "bold",
  "/": "italic",
  "=": "verbatim",
};

function isWhitespace(char: string | undefined): boolean {
  return char !== undefined && /\s/.test(char);
}

function isPreBoundary(char: string | undefined): boolean {
  return char === undefined || isWhitespace(char) || PRE_MARKERS.has(char);
}

function isPostBoundary(char: string | undefined): boolean {
  return char === undefined || isWhitespace(char) || POST_MARKERS.has(char);
}

const MATH_PAIRS: ReadonlyArray<readonly [string, string]> = [
  ["(", ")"],
  ["[", "]"],
];

/**
 * One scan over one run of text.
 *
 * The `unpartnered` maps memoize the smallest index from which a search for that
 * closing delimiter already ran to the end of the text and found nothing. A later
 * opener searches a suffix of that stretch, so it is answered from the memo,
 * keeping a run of many unpartnered openers out of quadratic time.
 */
class Scan {
  private readonly unpartneredMath = new Map<string, number>();
  private readonly unpartneredEmphasis = new Map<string, number>();

  constructor(private readonly text: string) {}

  /** Parse the whole run at nesting `depth` (see `MAX_EMPHASIS_DEPTH`). */
  parse(depth: number): Inline[] {
    const nodes: Inline[] = [];
    let buffer = "";
    let i = 0;

    const flush = (): void => {
      if (buffer.length > 0) {
        nodes.push({ type: "text", value: buffer });
        buffer = "";
      }
    };

    while (i < this.text.length) {
      const math = this.matchMath(i);
      if (math) {
        flush();
        nodes.push({ type: "math", tex: math.tex });
        i = math.end;
        continue;
      }
      const link = this.matchLink(i, depth);
      if (link) {
        flush();
        nodes.push(link.node);
        i = link.end;
        continue;
      }
      const emphasis = this.matchEmphasis(i, depth);
      if (emphasis) {
        flush();
        nodes.push(emphasis.node);
        i = emphasis.end;
        continue;
      }
      buffer += this.text[i];
      i += 1;
    }

    flush();
    return nodes;
  }

  /** Match a math span (`\(...\)` or `\[...\]`) opening at `i`. */
  private matchMath(i: number): { tex: string; end: number } | null {
    for (const [open, close] of MATH_PAIRS) {
      const openLength = mathDelimiterLength(this.text, i, open);
      if (openLength === 0) {
        continue;
      }
      const bodyStart = i + openLength;
      if (bodyStart >= (this.unpartneredMath.get(close) ?? Infinity)) {
        continue;
      }
      const closeAt = this.findMathClose(close, bodyStart);
      if (closeAt === null) {
        this.unpartneredMath.set(close, bodyStart);
        continue;
      }
      // A `\\(`/`\\[` open (length 3) is doubled-encoded; `\(`/`\[` (2) is not.
      const tex = normalizeTex(this.text.slice(bodyStart, closeAt.at), openLength === 3);
      // Empty delimiters carry no formula, so reporting no match leaves them as
      // literal text rather than asking KaTeX to typeset nothing.
      if (tex.length === 0) {
        return null;
      }
      return { tex, end: closeAt.at + closeAt.length };
    }
    return null;
  }

  private findMathClose(
    close: string,
    from: number,
  ): { at: number; length: number } | null {
    for (let j = from; j < this.text.length; j += 1) {
      const length = mathDelimiterLength(this.text, j, close);
      if (length > 0) {
        return { at: j, length };
      }
    }
    return null;
  }

  /** Match an Org link (`[[target]]` or `[[target][label]]`) opening at `i`. */
  private matchLink(i: number, depth: number): { node: Inline; end: number } | null {
    const text = this.text;
    if (text[i] !== "[" || text[i + 1] !== "[") {
      return null;
    }
    const targetStart = i + 2;
    const firstClose = text.indexOf("]", targetStart);
    if (firstClose === -1) {
      return null;
    }
    const target = text.slice(targetStart, firstClose);
    if (target.length === 0) {
      return null;
    }

    let label: readonly Inline[];
    let end: number;
    if (text[firstClose + 1] === "]") {
      // A bare `[[target]]`: the target doubles as its own label.
      label = [{ type: "text", value: target }];
      end = firstClose + 2;
    } else if (text[firstClose + 1] === "[") {
      const labelStart = firstClose + 2;
      const labelClose = text.indexOf("]]", labelStart);
      if (labelClose === -1) {
        return null;
      }
      label = nested(text.slice(labelStart, labelClose), depth + 1);
      end = labelClose + 2;
    } else {
      return null;
    }

    const id = target.startsWith("id:") ? target.slice(3) : null;
    return { node: { type: "link", target, id, label }, end };
  }

  /** Match an emphasis span opening at `i`, honoring Org's border rules. */
  private matchEmphasis(i: number, depth: number): { node: Inline; end: number } | null {
    const text = this.text;
    const marker = text[i]!;
    const kind = EMPHASIS[marker];
    if (kind === undefined || !isPreBoundary(text[i - 1])) {
      return null;
    }
    // The first body character must not be whitespace.
    if (isWhitespace(text[i + 1]) || text[i + 1] === undefined) {
      return null;
    }
    const bodyStart = i + 1;
    if (bodyStart >= (this.unpartneredEmphasis.get(marker) ?? Infinity)) {
      return null;
    }

    const literal = kind === "verbatim";
    for (let j = bodyStart; j < text.length; j += 1) {
      // A closing marker inside a nested math span or link is not a marker. In
      // verbatim nothing nests, so that scan stays raw.
      if (!literal) {
        const skipTo = this.protectedSpanEnd(j, depth);
        if (skipTo !== -1) {
          j = skipTo - 1;
          continue;
        }
      }
      if (text[j] !== marker) {
        continue;
      }
      // A valid close has a non-whitespace border before it and a boundary after.
      if (isWhitespace(text[j - 1]) || !isPostBoundary(text[j + 1])) {
        continue;
      }
      const inner = text.slice(bodyStart, j);
      const node: Inline = literal
        ? { type: "verbatim", value: inner }
        : { type: kind, children: nested(inner, depth + 1) };
      return { node, end: j + 1 };
    }
    this.unpartneredEmphasis.set(marker, bodyStart);
    return null;
  }

  /** The index just past a math span or link opening at `i`, else -1. */
  private protectedSpanEnd(i: number, depth: number): number {
    const link = this.matchLink(i, depth);
    if (link) {
      return link.end;
    }
    const math = this.matchMath(i);
    return math ? math.end : -1;
  }
}

/**
 * The nodes inside a span nested `depth` deep: the body's own inline content, or
 * the body as plain text once `MAX_EMPHASIS_DEPTH` is reached.
 */
function nested(text: string, depth: number): Inline[] {
  return depth >= MAX_EMPHASIS_DEPTH
    ? [{ type: "text", value: text }]
    : new Scan(text).parse(depth);
}

export function parseInline(text: string): Inline[] {
  return new Scan(text).parse(0);
}
