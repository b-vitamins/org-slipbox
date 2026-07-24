/*
 * Block-level Org parsing: a line scanner grouping a note's source into `Block`s;
 * everything inline defers to `parseInline`. A closing delimiter is searched for
 * before its construct is entered, so a closed construct is read whole and an
 * unclosed one costs its own line. In Org, indent is structure only within a list.
 */

import { parseInline } from "./parse-inline.js";
import { mathDelimiterLength, normalizeTex } from "./tex.js";
import type {
  Block,
  ListBlock,
  ListItem,
  OrgDocument,
  TableCell,
  TableRow,
} from "./types.js";

const HEADING = /^(\*+)\s+(.*)$/;
const KEYWORD = /^\s*#\+[A-Za-z_]+:/;
/** A comment line: `#` alone or followed by a space, and never `#+`. */
const COMMENT = /^\s*#(?:\s|$)/;
const DRAWER_OPEN = /^:[A-Za-z][A-Za-z0-9_-]*:$/;
const DRAWER_CLOSE = /^\s*:END:\s*$/i;
const UNORDERED_ITEM = /^(\s*)([-+])\s+(.*)$/;
const ORDERED_ITEM = /^(\s*)(\d+)[.)]\s+(.*)$/;
const SRC_OPEN = /^\s*#\+begin_src(?:\s+(\S+))?.*$/i;
const SRC_CLOSE = /^\s*#\+end_src\s*$/i;
const EXAMPLE_OPEN = /^\s*#\+begin_example.*$/i;
const EXAMPLE_CLOSE = /^\s*#\+end_example\s*$/i;
/** Any `#+begin_NAME` line, whether or not the model has a block for it. */
const BLOCK_OPEN = /^\s*#\+begin_([A-Za-z][A-Za-z0-9_-]*)/i;
/** A `\]` or `\\]` display-math close, wherever it sits on its line. */
const DISPLAY_MATH_CLOSE = /\\\\?\]/;
/** A LaTeX display environment (`\begin{align*}`) standing on its own line. */
const MATH_ENV_OPEN = /^\s*\\\\?begin\{([A-Za-z]+\*?)\}\s*$/;
const TABLE_ROW = /^\s*\|(.*)$/;

class LineCursor {
  private index = 0;

  constructor(private readonly lines: readonly string[]) {}

  atEnd(): boolean {
    return this.index >= this.lines.length;
  }

  peek(): string | undefined {
    return this.lines[this.index];
  }

  peekAhead(offset: number): string | undefined {
    return this.lines[this.index + offset];
  }

  next(): string {
    const line = this.lines[this.index] ?? "";
    this.index += 1;
    return line;
  }

  take(count: number): string[] {
    const taken = this.lines.slice(this.index, this.index + count);
    this.index += count;
    return [...taken];
  }

  skip(count: number): void {
    this.index += count;
  }
}

/**
 * How far past the cursor the first line matching `close` lies, starting the
 * search `from` lines ahead, or null if no such line does. A line matching `stop`
 * ends the construct's scope, so a `close` beyond it belongs to something else.
 */
function distanceTo(
  cursor: LineCursor,
  from: number,
  close: RegExp,
  stop?: RegExp,
): number | null {
  for (let ahead = from; ; ahead += 1) {
    const line = cursor.peekAhead(ahead);
    if (line === undefined) {
      return null;
    }
    if (close.test(line)) {
      return ahead;
    }
    if (stop?.test(line)) {
      return null;
    }
  }
}

/**
 * Consume a `:DRAWER:` to `:END:` drawer at the cursor, if one closes there.
 *
 * An Org drawer belongs to the entity it sits under, so the `:END:` search stops
 * at the next heading. With no close the line is not a drawer and is left for the
 * scanner to read as prose.
 */
function skipDrawer(cursor: LineCursor): boolean {
  const line = cursor.peek()?.trim() ?? "";
  if (!DRAWER_OPEN.test(line) || DRAWER_CLOSE.test(line)) {
    return false;
  }
  const end = distanceTo(cursor, 1, DRAWER_CLOSE, HEADING);
  if (end === null) {
    return false;
  }
  cursor.skip(end + 1);
  return true;
}

function indentOf(line: string): number {
  return line.length - line.trimStart().length;
}

interface ItemMatch {
  readonly indent: number;
  readonly ordered: boolean;
  readonly number: number;
  readonly text: string;
}

function matchItem(line: string): ItemMatch | null {
  const unordered = UNORDERED_ITEM.exec(line);
  if (unordered) {
    return {
      indent: unordered[1]!.length,
      ordered: false,
      number: 1,
      text: unordered[3] ?? "",
    };
  }
  const ordered = ORDERED_ITEM.exec(line);
  if (ordered) {
    return {
      indent: ordered[1]!.length,
      ordered: true,
      number: Number.parseInt(ordered[2]!, 10),
      text: ordered[3] ?? "",
    };
  }
  return null;
}

function pastBlanks(cursor: LineCursor): number | null {
  let ahead = 0;
  while (true) {
    const line = cursor.peekAhead(ahead);
    if (line === undefined) {
      return null;
    }
    if (line.trim() !== "") {
      return ahead;
    }
    ahead += 1;
  }
}

/**
 * The item a list continues with past a run of blank lines, or null if the run
 * ends the list. In Org a blank line inside a list is a gap between items, so
 * `1.`/blank/`2.` is one list; an item indented less than `indent` belongs to an
 * enclosing list and closes this one.
 */
function itemPastBlanks(cursor: LineCursor, indent: number): number | null {
  const ahead = pastBlanks(cursor);
  if (ahead === null) {
    return null;
  }
  const item = matchItem(cursor.peekAhead(ahead)!);
  if (item === null || item.indent < indent) {
    return null;
  }
  return ahead;
}

/**
 * How far past the cursor a nested run resumes after blank lines, or null if it
 * does not: content indented past `enclosing` still belongs to the item.
 */
function contentPastBlanks(cursor: LineCursor, enclosing: number): number | null {
  const ahead = pastBlanks(cursor);
  if (ahead === null || indentOf(cursor.peekAhead(ahead)!) <= enclosing) {
    return null;
  }
  return ahead;
}

/**
 * Collect a list of items indented at `indent`. `ordered` is fixed by the first
 * item, so a change of marker style starts a new list, and `start` is that item's
 * number, so a resuming list keeps counting.
 */
function parseList(cursor: LineCursor, indent: number, ordered: boolean): ListBlock {
  const items: ListItem[] = [];
  let start = 1;
  while (!cursor.atEnd()) {
    const line = cursor.peek() ?? "";
    const item = matchItem(line);
    if (item === null || item.indent !== indent || item.ordered !== ordered) {
      if (line.trim() !== "") {
        break;
      }
      const ahead = itemPastBlanks(cursor, indent);
      if (ahead === null) {
        break;
      }
      cursor.skip(ahead);
      continue;
    }
    cursor.next();
    if (items.length === 0) {
      start = item.number;
    }
    items.push(parseItem(cursor, item));
  }
  return { type: "list", ordered, start, items };
}

/**
 * Collect one item: its marker line joined with the lines wrapped directly beneath
 * it, so a construct broken across lines stays whole, then the nested blocks.
 */
function parseItem(cursor: LineCursor, item: ItemMatch): ListItem {
  const text: string[] = [item.text];
  while (!cursor.atEnd()) {
    const line = cursor.peek() ?? "";
    if (line.trim() === "" || indentOf(line) <= item.indent || startsBlock(line)) {
      break;
    }
    text.push(line.trim());
    cursor.next();
  }
  return {
    children: parseInline(text.join(" ")),
    blocks: scanBlocks(cursor, item.indent),
  };
}

/** Append a math block unless it is empty, which KaTeX would set as a blank gap. */
function pushMath(blocks: Block[], math: Block): void {
  if (math.type === "math" && math.tex.length === 0) {
    return;
  }
  blocks.push(math);
}

function startsBlock(line: string): boolean {
  return (
    matchItem(line) !== null ||
    KEYWORD.test(line) ||
    HEADING.test(line) ||
    BLOCK_OPEN.test(line) ||
    matchDisplayMath(line) !== null ||
    MATH_ENV_OPEN.test(line) ||
    TABLE_ROW.test(line) ||
    DRAWER_OPEN.test(line.trim())
  );
}

/**
 * Collect a fenced block (`src`/`example`) verbatim, until its close line.
 *
 * The close is located first, so everything before it is content whatever it looks
 * like. Org fences of one kind do not nest (an inner opener is comma-escaped), so
 * the search stops at a second `open`, whose close is its own.
 *
 * An unclosed fence ends at the next heading or fence opening. A blank line is
 * ordinary fence content and cannot serve as the boundary.
 */
function parseFenced(cursor: LineCursor, open: RegExp, close: RegExp): string {
  const end = distanceTo(cursor, 0, close, open);
  if (end !== null) {
    const body = cursor.take(end);
    cursor.skip(1);
    return body.join("\n");
  }
  const body: string[] = [];
  while (!cursor.atEnd()) {
    const line = cursor.peek() ?? "";
    if (HEADING.test(line) || SRC_OPEN.test(line) || EXAMPLE_OPEN.test(line)) {
      break;
    }
    body.push(cursor.next());
  }
  return body.join("\n");
}

/**
 * A display-math block opening on `line`: `\[` at the start of the line, the body
 * following it there, and whether the line also closes it.
 *
 * `\[...\]` is display math wherever it appears, so a formula written on one line
 * is a block. A line that closes and then carries on is prose with math in it and
 * is left to the paragraph path, so it is not matched here.
 */
interface DisplayMathOpen {
  readonly doubled: boolean;
  readonly body: string;
  readonly closed: boolean;
}

function matchDisplayMath(line: string): DisplayMathOpen | null {
  const trimmed = line.trimStart();
  const openLength = mathDelimiterLength(trimmed, 0, "[");
  if (openLength === 0) {
    return null;
  }
  const rest = trimmed.slice(openLength);
  const close = DISPLAY_MATH_CLOSE.exec(rest);
  const doubled = openLength === 3;
  if (close === null) {
    return { doubled, body: rest, closed: false };
  }
  if (rest.slice(close.index + close[0].length).trim() !== "") {
    return null;
  }
  return { doubled, body: rest.slice(0, close.index), closed: true };
}

/** Collect a `\[ ... \]` block from its opening line, whose text `open` carries. */
function parseDisplayMath(cursor: LineCursor, open: DisplayMathOpen): Block {
  const body = [open.body];
  if (!open.closed) {
    body.push(...collectMath(cursor, DISPLAY_MATH_CLOSE).body);
  }
  return { type: "math", tex: normalizeTex(body.join("\n"), open.doubled) };
}

/**
 * Collect a LaTeX display environment, delimiters included: KaTeX reads
 * `\begin{align*}` itself, so the environment is handed over whole.
 */
function parseMathEnvironment(cursor: LineCursor, environment: string, doubled: boolean): Block {
  const open = cursor.next().trim();
  // An environment name may end in `*`, a regex quantifier unless escaped.
  const name = environment.replace(/\*/g, "\\*");
  const close = new RegExp(`\\\\\\\\?end\\{${name}\\}`);
  const collected = collectMath(cursor, close);
  const raw = [open, ...collected.body, collected.close ?? `\\end{${environment}}`]
    .filter((part) => part.length > 0)
    .join("\n");
  return { type: "math", tex: normalizeTex(raw, doubled) };
}

interface CollectedMath {
  readonly body: readonly string[];
  readonly close: string | null;
}

/**
 * Collect math body lines up to `close`, which may sit at the end of a body line
 * rather than alone on one; keeping it in the body hands KaTeX a stray `\]`.
 *
 * A blank line also ends the run, bounding math whose close is missing.
 */
function collectMath(cursor: LineCursor, close: RegExp): CollectedMath {
  const body: string[] = [];
  while (!cursor.atEnd()) {
    const line = cursor.peek() ?? "";
    const closes = close.exec(line);
    if (closes) {
      cursor.next();
      const before = line.slice(0, closes.index).trim();
      if (before.length > 0) {
        body.push(before);
      }
      return { body, close: closes[0] };
    }
    if (line.trim() === "") {
      break;
    }
    body.push(cursor.next());
  }
  return { body, close: null };
}

/** A rule row: Org's `|---+---|`, which carries no content. */
function isRule(body: string): boolean {
  return /^-[\s|+-]*$/.test(body);
}

function parseCells(body: string): TableCell[] {
  return body
    .replace(/\|\s*$/, "")
    .split("|")
    .map((cell) => ({ children: parseInline(cell.trim()) }));
}

/**
 * Collect a contiguous run of `|`-delimited rows into a table, grouped by the
 * `|---|` rules between them. Org's header is the first group a rule separates
 * from a group of body rows: a table with no rule is all body, and a boxed table
 * (one opening with a rule) has its header between its first two rules.
 */
function parseTable(cursor: LineCursor): Block {
  const groups: TableCell[][][] = [[]];
  let ruled = false;
  while (!cursor.atEnd()) {
    const match = TABLE_ROW.exec(cursor.peek() ?? "");
    if (!match) {
      break;
    }
    cursor.next();
    const body = match[1] ?? "";
    if (isRule(body)) {
      ruled = true;
      if (groups[groups.length - 1]!.length > 0) {
        groups.push([]);
      }
      continue;
    }
    if (body.trim() === "") {
      continue;
    }
    groups[groups.length - 1]!.push(parseCells(body));
  }
  const filled = groups.filter((group) => group.length > 0);
  const header = ruled && filled.length > 1 ? filled[0]! : [];
  const body = header.length > 0 ? filled.slice(1) : filled;
  const rows: TableRow[] = [
    ...header.map((cells) => ({ header: true, cells })),
    ...body.flat().map((cells) => ({ header: false, cells })),
  ];
  return { type: "table", rows };
}

/**
 * Collect a `#+begin_NAME` to `#+end_NAME` wrapper the model has no block for,
 * reduced to what it contributes.
 *
 * An Org `comment` block is not for display and contributes nothing. Every other
 * wrapper contributes its body parsed as blocks; its delimiter lines are
 * directives, not prose.
 */
function parseWrapper(cursor: LineCursor, name: string): Block[] {
  const close = new RegExp(`^\\s*#\\+end_${name}\\s*$`, "i");
  cursor.next();
  const end = distanceTo(cursor, 0, close);
  const body = end === null ? [] : cursor.take(end);
  if (end !== null) {
    cursor.skip(1);
  }
  if (name.toLowerCase() === "comment") {
    return [];
  }
  const blocks = scanBlocks(new LineCursor(body), -1);
  if (name.toLowerCase() === "quote") {
    return blocks.length > 0 ? [{ type: "quote", blocks }] : [];
  }
  return blocks;
}

/**
 * Scan blocks until the source runs out, or, where `enclosing` is a list item's
 * indent, until a line is indented no further than it. A blank line separates
 * blocks, and within a nested run ends it unless a deeper indent follows.
 */
function scanBlocks(cursor: LineCursor, enclosing: number): Block[] {
  const blocks: Block[] = [];
  let paragraph: string[] = [];

  const flushParagraph = (): void => {
    if (paragraph.length > 0) {
      blocks.push({ type: "paragraph", children: parseInline(paragraph.join(" ")) });
      paragraph = [];
    }
  };

  while (!cursor.atEnd()) {
    const line = cursor.peek() ?? "";

    if (line.trim() === "") {
      flushParagraph();
      if (enclosing >= 0 && contentPastBlanks(cursor, enclosing) === null) {
        break;
      }
      cursor.next();
      continue;
    }
    if (enclosing >= 0 && indentOf(line) <= enclosing) {
      break;
    }
    if (KEYWORD.test(line)) {
      cursor.next();
      flushParagraph();
      continue;
    }
    // A comment does not divide a paragraph, so the lines around one join.
    if (COMMENT.test(line)) {
      cursor.next();
      continue;
    }
    // A drawer is metadata wherever it appears, not just as leading
    // `:PROPERTIES:`, but a `:` line mid-paragraph is prose.
    if (paragraph.length === 0 && skipDrawer(cursor)) {
      continue;
    }

    const heading = HEADING.exec(line);
    if (heading) {
      cursor.next();
      flushParagraph();
      blocks.push({
        type: "heading",
        level: heading[1]!.length,
        children: parseInline(heading[2] ?? ""),
      });
      continue;
    }

    if (SRC_OPEN.test(line)) {
      flushParagraph();
      const lang = SRC_OPEN.exec(line)?.[1] ?? null;
      cursor.next();
      blocks.push({
        type: "src",
        lang,
        code: parseFenced(cursor, SRC_OPEN, SRC_CLOSE),
      });
      continue;
    }
    if (EXAMPLE_OPEN.test(line)) {
      flushParagraph();
      cursor.next();
      blocks.push({
        type: "example",
        text: parseFenced(cursor, EXAMPLE_OPEN, EXAMPLE_CLOSE),
      });
      continue;
    }
    const wrapper = BLOCK_OPEN.exec(line);
    if (wrapper) {
      flushParagraph();
      blocks.push(...parseWrapper(cursor, wrapper[1]!));
      continue;
    }
    const displayMath = matchDisplayMath(line);
    if (displayMath) {
      flushParagraph();
      cursor.next();
      pushMath(blocks, parseDisplayMath(cursor, displayMath));
      continue;
    }
    const environment = MATH_ENV_OPEN.exec(line);
    if (environment) {
      flushParagraph();
      pushMath(
        blocks,
        parseMathEnvironment(cursor, environment[1]!, line.trimStart().startsWith("\\\\")),
      );
      continue;
    }
    const item = matchItem(line);
    if (item) {
      flushParagraph();
      blocks.push(parseList(cursor, item.indent, item.ordered));
      continue;
    }
    if (TABLE_ROW.test(line)) {
      flushParagraph();
      blocks.push(parseTable(cursor));
      continue;
    }

    paragraph.push(line.trim());
    cursor.next();
  }

  flushParagraph();
  return blocks;
}

/** Parse a note's raw Org source into a document model. */
export function parseOrg(source: string): OrgDocument {
  // Split on all three line endings: every construct is a whole-line shape, and a
  // trailing carriage return would defeat each pattern's `$` anchor.
  return { blocks: scanBlocks(new LineCursor(source.split(/\r\n|\r|\n/)), -1) };
}
