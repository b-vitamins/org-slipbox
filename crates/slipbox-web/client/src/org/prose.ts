/*
 * Preview prose: a bounded run of inline nodes, never a flattened string, so a
 * formula is typeset rather than shown as TeX. A link contributes its label only,
 * blocks join into one line with display math inline, and abridging counts
 * characters, cutting running text on a word boundary and atoms not at all.
 */

import type { Block, Inline, OrgDocument } from "./types.js";

/** Grapheme segmenter, so a cut never lands inside a character's encoding. */
const GRAPHEMES = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/** Append `node`, merging it into a preceding text node and dropping empties. */
function append(prose: Inline[], node: Inline): void {
  if (node.type === "text" && node.value.length === 0) {
    return;
  }
  const last = prose[prose.length - 1];
  if (node.type === "text" && last?.type === "text") {
    prose[prose.length - 1] = { type: "text", value: last.value + node.value };
    return;
  }
  prose.push(node);
}

function textProse(raw: string): Inline[] {
  const collapsed = raw.replace(/\s+/g, " ");
  return collapsed.length > 0 ? [{ type: "text", value: collapsed }] : [];
}

function joinProse(runs: readonly Inline[][]): Inline[] {
  const prose: Inline[] = [];
  for (const run of runs) {
    if (run.length === 0) {
      continue;
    }
    if (prose.length > 0) {
      append(prose, { type: "text", value: " " });
    }
    for (const node of run) {
      append(prose, node);
    }
  }
  return prose;
}

/** Reduce a run of inline nodes to prose: links unwrapped, whitespace collapsed. */
function inlineProse(nodes: readonly Inline[]): Inline[] {
  const prose: Inline[] = [];
  for (const node of nodes) {
    switch (node.type) {
      case "text":
        for (const text of textProse(node.value)) {
          append(prose, text);
        }
        break;
      case "bold":
      case "italic":
        prose.push({ type: node.type, children: inlineProse(node.children) });
        break;
      case "link":
        for (const label of inlineProse(node.label)) {
          append(prose, label);
        }
        break;
      default:
        append(prose, node);
    }
  }
  return prose;
}

function blockProse(block: Block): Inline[] {
  switch (block.type) {
    case "heading":
    case "paragraph":
      return inlineProse(block.children);
    case "list":
      return joinProse(
        block.items.flatMap((item) => [
          inlineProse(item.children),
          ...item.blocks.map(blockProse),
        ]),
      );
    case "src":
      return textProse(block.code);
    case "example":
      return textProse(block.text);
    case "quote":
      return joinProse(block.blocks.map(blockProse));
    case "math":
      return [{ type: "math", tex: block.tex }];
    case "table":
      return joinProse(
        block.rows.flatMap((row) =>
          row.cells.map((cell) => inlineProse(cell.children)),
        ),
      );
  }
}

function trim(prose: readonly Inline[]): Inline[] {
  const trimmed = [...prose];
  const first = trimmed[0];
  if (first?.type === "text") {
    trimmed[0] = { type: "text", value: first.value.trimStart() };
  }
  const lastIndex = trimmed.length - 1;
  const last = trimmed[lastIndex];
  if (last?.type === "text") {
    trimmed[lastIndex] = { type: "text", value: last.value.trimEnd() };
  }
  return trimmed.filter((node) => node.type !== "text" || node.value.length > 0);
}

/**
 * How much of a preview's character budget is left, whether any was cut, and
 * whether anything has been kept yet. All three belong to the whole preview: a
 * nested run shares one `Budget` object with the run containing it, so `taken`
 * gates the over-wide-atom concession once rather than once per nesting level.
 */
interface Budget {
  left: number;
  elided: boolean;
  taken: boolean;
}

/**
 * What an atomic node costs against the budget, or 0 for a node the budget is
 * spent inside of. Math and verbatim are atomic: one either fits or is left out.
 */
function atomWidth(node: Inline): number {
  switch (node.type) {
    case "math":
      return node.tex.length;
    case "verbatim":
      return node.value.length;
    default:
      return 0;
  }
}

/** Take prose from `nodes` while `budget` allows, cutting only running text. */
function take(nodes: readonly Inline[], budget: Budget): Inline[] {
  const kept: Inline[] = [];
  for (const node of nodes) {
    if (budget.left <= 0) {
      budget.elided = true;
      break;
    }
    // An atom wider than the whole budget is kept whole while nothing has been
    // taken yet, so a preview opening on a formula is not just an ellipsis. Once
    // per preview, since `taken` is shared with every nested run.
    if (!budget.taken && atomWidth(node) > budget.left) {
      budget.left = 0;
      budget.elided = true;
      budget.taken = true;
      kept.push(node);
      continue;
    }
    if (node.type === "text") {
      if (node.value.length <= budget.left) {
        budget.left -= node.value.length;
        budget.taken = budget.taken || node.value.length > 0;
        append(kept, node);
        continue;
      }
      const cut = cutWords(node.value, budget.left);
      append(kept, { type: "text", value: cut });
      budget.left = 0;
      budget.elided = true;
      budget.taken = budget.taken || cut.length > 0;
      break;
    }
    // Emphasis wraps prose, so the budget is spent inside it and an emphasized
    // run is cut like any other rather than dropped whole.
    if (node.type === "bold" || node.type === "italic") {
      const children = take(node.children, budget);
      if (children.length > 0) {
        kept.push({ type: node.type, children });
      }
      continue;
    }
    if (node.type === "link") {
      for (const label of take(node.label, budget)) {
        append(kept, label);
      }
      continue;
    }
    const cost = atomWidth(node);
    if (cost > budget.left) {
      budget.elided = true;
      break;
    }
    budget.left -= cost;
    budget.taken = true;
    kept.push(node);
  }
  return kept;
}

/**
 * The longest whole-word prefix of `text` within `limit` characters, falling back
 * to a grapheme boundary where the prefix holds no space, so an emoji or a
 * combining sequence is never split.
 */
function cutWords(text: string, limit: number): string {
  const clipped = clipGraphemes(text, limit);
  const lastSpace = clipped.lastIndexOf(" ");
  const cut = lastSpace > 0 ? clipped.slice(0, lastSpace) : clipped;
  return cut.trimEnd();
}

function clipGraphemes(text: string, limit: number): string {
  if (text.length <= limit) {
    return text;
  }
  let kept = 0;
  for (const { segment } of GRAPHEMES.segment(text)) {
    if (kept + segment.length > limit) {
      break;
    }
    kept += segment.length;
  }
  return text.slice(0, kept);
}

/** Bound `prose` to `maxChars`, marking any elision with a trailing ellipsis. */
function abridge(prose: readonly Inline[], maxChars: number): Inline[] {
  const budget: Budget = { left: maxChars, elided: false, taken: false };
  const kept = take(prose, budget);
  if (!budget.elided) {
    return kept;
  }
  const marked = trim(kept);
  append(marked, { type: "text", value: "…" });
  return marked;
}

/**
 * Preview prose for a run of inline nodes. `maxChars` bounds it; omit the bound
 * for chrome that clips its own single line in CSS.
 */
export function inlinePreview(
  nodes: readonly Inline[],
  maxChars = Number.POSITIVE_INFINITY,
): Inline[] {
  return abridge(trim(inlineProse(nodes)), maxChars);
}

/** Preview prose for a document, its blocks joined into one bounded line. */
export function documentPreview(
  document: OrgDocument,
  maxChars: number,
): Inline[] {
  return abridge(trim(joinProse(document.blocks.map(blockProse))), maxChars);
}
