/*
 * A content-search excerpt reduced to preview prose. The server flags matched runs of
 * Org source, whose offsets the reduction invalidates, so the flags ride through the
 * parse as two control characters spliced around each run and passed through as text.
 * Both are stripped from each incoming segment first, so a note cannot forge one.
 */

import type { ContentSegment } from "../api/types.js";
import { parseInline } from "../org/parse-inline.js";
import { inlinePreview } from "../org/prose.js";
import type { Inline } from "../org/types.js";

/** Opens a matched run in the spliced source (U+0002, start of text). */
const OPEN = "\u0002";
/** Closes a matched run in the spliced source (U+0003, end of text). */
const CLOSE = "\u0003";
/** Both markers, for stripping them from text that is not the splice. */
const MARKERS = /[\u0002\u0003]/g;

/** The emphasis kinds a leaf can sit inside of. */
type Wrapper = "bold" | "italic";

/** How the server marks its own elision, and what a cut edge is marked with. */
const ELLIPSIS = "…";

/**
 * A construct an excerpt can be cut in half. `divider` is where the construct
 * stops being machinery and starts being words: a link's is `][`, and math has
 * none, being TeX from opener to closer.
 *
 * Emphasis is absent: Org already reads an unpartnered `/` or `=` as the character
 * it spells.
 */
interface Construct {
  readonly open: string;
  readonly close: string;
  readonly divider: string | null;
}

const CONSTRUCTS: readonly Construct[] = [
  { open: "[[", close: "]]", divider: "][" },
  { open: "\\(", close: "\\)", divider: null },
  { open: "\\[", close: "\\]", divider: null },
];

/**
 * Where an excerpt stops being prose, or null when it runs whole to its end.
 *
 * An excerpt is a slice, so the server's elision can leave an opener with no
 * partner, and the parser reads an unclosed `[[` or `\(` as the characters it
 * spells.
 */
function cutPoint(source: string): number | null {
  let cut: number | null = null;
  for (const { open, close } of CONSTRUCTS) {
    const at = source.lastIndexOf(open);
    if (at === -1 || source.includes(close, at + open.length)) {
      continue;
    }
    cut = cut === null ? at : Math.min(cut, at);
  }
  return cut;
}

/**
 * End an excerpt at a construct the elision cut in half, marking the cut with the
 * server's own ellipsis and closing any highlight left open there.
 */
function dropCutTail(source: string): string {
  const cut = cutPoint(source);
  if (cut === null) {
    return source;
  }
  const kept = source
    .slice(0, cut)
    // A doubled-escaped opener (`\\(`) leaves its first backslash behind.
    .replace(/\\+$/, "")
    .trimEnd();
  const open = kept.split(OPEN).length - 1;
  const closed = kept.split(CLOSE).length - 1;
  return `${kept}${CLOSE.repeat(Math.max(0, open - closed))}${ELLIPSIS}`;
}

/**
 * Where an excerpt starts being prose, or null when it runs whole from its start.
 *
 * The mirror of `cutPoint`. Only closers standing before the first opener of their
 * kind are debris, and the last of them is the cut: cutting at an earlier one
 * would leave the next still unpartnered.
 */
function headCut(source: string): (Construct & { readonly at: number }) | null {
  let cut: (Construct & { readonly at: number }) | null = null;
  for (const construct of CONSTRUCTS) {
    const opened = source.indexOf(construct.open);
    const limit = opened === -1 ? source.length : opened;
    const at = source.lastIndexOf(construct.close, limit - construct.close.length);
    if (at === -1) {
      continue;
    }
    if (cut === null || at > cut.at) {
      cut = { ...construct, at };
    }
  }
  return cut;
}

/**
 * Start an excerpt after a construct the elision cut in half, marking the cut.
 *
 * What precedes the stray closer is kept where it is words. For a link a surviving
 * `][` says the cut took part of the target, so the description after it is intact;
 * with none the cut fell inside the description, which is already words. A
 * highlight the drop left half-open is reopened at the words.
 */
function dropCutHead(source: string): string {
  const cut = headCut(source);
  if (cut === null) {
    return source;
  }
  const head = source.slice(0, cut.at);
  let words = "";
  if (cut.divider !== null) {
    const divided = head.lastIndexOf(cut.divider);
    words = divided === -1 ? head : head.slice(divided + cut.divider.length);
    words = words
      // The server's own elision mark, replaced by the one this cut stands for.
      .replace(/^…/, "")
      // Org reads a stray `[` as the character it spells: a cut between the two
      // brackets leaves the description's own bracket with nothing to open.
      .replace(/^\[/, "")
      .trimStart();
  }
  const rest = source.slice(cut.at + cut.close.length);
  const kept = `${words}${words === "" ? rest.trimStart() : rest}`;
  const open = kept.split(OPEN).length - 1;
  const closed = kept.split(CLOSE).length - 1;
  return `${ELLIPSIS}${OPEN.repeat(Math.max(0, closed - open))}${kept}`;
}

// Approximate the two-line CSS clamp while retaining it as the final visual guard.
const LINE_CHARACTERS = 84;
const CLAMPED_LINES = 2;
const DRAWN_CHARACTERS = LINE_CHARACTERS * CLAMPED_LINES;

function drawnLength(nodes: readonly Inline[]): number {
  let length = 0;
  for (const node of nodes) {
    switch (node.type) {
      case "text":
      case "verbatim":
        length += node.value.length;
        break;
      case "math":
        length += node.tex.length;
        break;
      case "bold":
      case "italic":
        length += drawnLength(node.children);
        break;
      case "link":
        length += drawnLength(node.label);
        break;
    }
  }
  return length;
}

function drawn(source: string): number {
  return drawnLength(inlinePreview(parseInline(source.replace(MARKERS, ""))));
}

/** Find the outermost construct containing a proposed cut. */
function enclosingOpener(source: string, at: number): number | null {
  let inside: Construct | null = null;
  let opener = 0;
  for (let index = 0; index < source.length; index += 1) {
    if (inside === null) {
      const construct = CONSTRUCTS.find((one) => source.startsWith(one.open, index));
      if (construct === undefined) {
        continue;
      }
      if (index >= at) {
        return null;
      }
      inside = construct;
      opener = index;
      index += construct.open.length - 1;
      continue;
    }
    if (!source.startsWith(inside.close, index)) {
      continue;
    }
    const past = index + inside.close.length;
    if (past > at) {
      return opener;
    }
    inside = null;
    index = past - 1;
  }
  return inside === null ? null : opener;
}

function leadCut(source: string): number | null {
  const match = source.indexOf(OPEN);
  if (match === -1 || drawn(source) <= DRAWN_CHARACTERS) {
    return null;
  }
  if (drawn(source.slice(0, match)) <= LINE_CHARACTERS) {
    return null;
  }
  const at = match - LINE_CHARACTERS;
  const word = source.indexOf(" ", at);
  const between = word === -1 || word >= match ? at : word + 1;
  // Never begin inside Org markup; move the cut to its outer opener.
  return enclosingOpener(source, between) ?? between;
}

function dropLongLead(source: string): string {
  const cut = leadCut(source);
  if (cut === null) {
    return source;
  }
  return `${ELLIPSIS}${source.slice(cut).trimStart()}`;
}

/**
 * One run of an excerpt: a piece of preview prose, and whether the search matched
 * in it. Runs concatenate, in order, to the whole excerpt.
 */
export interface ExcerptRun {
  readonly matched: boolean;
  readonly prose: readonly Inline[];
}

/**
 * Splice the server's flags into one source string as marker characters.
 *
 * Neighbours carrying the same flag are joined first: the server emits one segment
 * per query word, so bracketing each separately would draw one highlight per word
 * instead of one over the phrase.
 */
function spliceMarkers(segments: readonly ContentSegment[]): string {
  let spliced = "";
  for (let index = 0; index < segments.length; index += 1) {
    const { matched } = segments[index]!;
    let text = "";
    while (index < segments.length && segments[index]!.matched === matched) {
      text += segments[index]!.text.replace(MARKERS, "");
      index += 1;
    }
    index -= 1;
    spliced += matched ? `${OPEN}${text}${CLOSE}` : text;
  }
  return spliced;
}

/** A leaf of the reduced prose, with the emphasis it was nested in. */
interface Leaf {
  readonly wrappers: readonly Wrapper[];
  readonly node: Inline;
}

/**
 * Rebuild a prose tree from leaves and the emphasis each sat inside. Leaves sharing
 * a wrapper at `depth` are grouped under one span.
 */
function rebuild(leaves: readonly Leaf[], depth: number): Inline[] {
  const nodes: Inline[] = [];
  let index = 0;
  while (index < leaves.length) {
    const wrapper = leaves[index]!.wrappers[depth];
    if (wrapper === undefined) {
      append(nodes, leaves[index]!.node);
      index += 1;
      continue;
    }
    let end = index + 1;
    while (end < leaves.length && leaves[end]!.wrappers[depth] === wrapper) {
      end += 1;
    }
    nodes.push({
      type: wrapper,
      children: rebuild(leaves.slice(index, end), depth + 1),
    });
    index = end;
  }
  return nodes;
}

function append(nodes: Inline[], node: Inline): void {
  if (node.type === "text" && node.value.length === 0) {
    return;
  }
  const last = nodes[nodes.length - 1];
  if (node.type === "text" && last?.type === "text") {
    nodes[nodes.length - 1] = { type: "text", value: last.value + node.value };
    return;
  }
  nodes.push(node);
}

/**
 * Splits reduced prose into runs at the markers. A marker only ends a run when it
 * flips the flag, so nested markers read as one highlight.
 */
class Split {
  private readonly runs: ExcerptRun[] = [];
  private leaves: Leaf[] = [];
  private readonly wrappers: Wrapper[] = [];
  private matched = false;
  /** How many markers are open; the run is matched while this is above zero. */
  private depth = 0;

  static run(nodes: readonly Inline[]): ExcerptRun[] {
    const split = new Split();
    split.walk(nodes);
    split.close();
    return split.runs;
  }

  private walk(nodes: readonly Inline[]): void {
    for (const node of nodes) {
      switch (node.type) {
        case "text":
          this.text(node.value);
          break;
        case "bold":
        case "italic":
          this.wrappers.push(node.type);
          this.walk(node.children);
          this.wrappers.pop();
          break;
        case "verbatim":
          this.leaf({ type: "verbatim", value: strip(node.value) });
          break;
        case "math":
          this.leaf({ type: "math", tex: strip(node.tex) });
          break;
        default:
          this.leaf(node);
      }
    }
  }

  private text(value: string): void {
    let text = "";
    for (const char of value) {
      if (char !== OPEN && char !== CLOSE) {
        text += char;
        continue;
      }
      this.leaf({ type: "text", value: text });
      text = "";
      this.mark(char);
    }
    this.leaf({ type: "text", value: text });
  }

  private mark(marker: string): void {
    this.depth =
      marker === OPEN ? this.depth + 1 : Math.max(0, this.depth - 1);
    const matched = this.depth > 0;
    if (matched === this.matched) {
      return;
    }
    this.close();
    this.matched = matched;
  }

  private leaf(node: Inline): void {
    if (node.type === "text" && node.value.length === 0) {
      return;
    }
    this.leaves.push({ wrappers: [...this.wrappers], node });
  }

  private close(): void {
    if (this.leaves.length === 0) {
      return;
    }
    this.runs.push({ matched: this.matched, prose: rebuild(this.leaves, 0) });
    this.leaves = [];
  }
}

/** Drop markers from an atom, which highlights whole or not at all. */
function strip(text: string): string {
  return text.replace(MARKERS, "");
}

/**
 * The runs of prose a content-search excerpt draws as, in order.
 */
export function excerptRuns(
  segments: readonly ContentSegment[],
): ExcerptRun[] {
  const source = dropLongLead(dropCutTail(dropCutHead(spliceMarkers(segments))));
  return Split.run(inlinePreview(parseInline(source)));
}
