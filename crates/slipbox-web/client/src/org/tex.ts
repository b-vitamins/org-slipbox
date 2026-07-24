/*
 * The corpus stores LaTeX doubled (`\\(\\sum\\)`, row break `\\\\`) or single
 * (`\(\sum\)`, `\\`), uniform per note. KaTeX wants single, so a doubled span is
 * collapsed and a single one passes through: collapsing it would fuse a row break
 * into the next command (`\\\xi` to `\\xi`). `doubled` comes from the delimiter.
 */

/** Collapse a doubled span's backslash pairs, then trim, for KaTeX. */
export function normalizeTex(raw: string, doubled: boolean): string {
  const collapsed = doubled ? raw.replace(/\\\\/g, "\\") : raw;
  return collapsed.trim();
}

/**
 * The length of a math delimiter (`\(`, `\)`, `\[`, `\]`) at `index`, allowing
 * the leading backslash to be singled or doubled, or `0` if none is present.
 * `bracket` is the delimiter's closing glyph: `(`, `)`, `[`, or `]`.
 */
export function mathDelimiterLength(text: string, index: number, bracket: string): number {
  if (text[index] !== "\\") {
    return 0;
  }
  if (text[index + 1] === "\\") {
    return text[index + 2] === bracket ? 3 : 0;
  }
  return text[index + 1] === bracket ? 2 : 0;
}
