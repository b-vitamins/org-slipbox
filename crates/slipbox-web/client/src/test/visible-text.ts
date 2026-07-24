/*
 * The text a reader sees inside a rendered fragment.
 *
 * KaTeX emits every formula twice: typeset glyphs, plus a MathML annotation
 * carrying the original TeX for assistive tech. `textContent` reads both, so the
 * MathML copy is dropped here and whitespace collapsed.
 */

/** The visible text of `root`, with KaTeX's MathML annotation excluded. */
export function visibleText(root: Element | null | undefined): string {
  if (!root) {
    return "";
  }
  const visible = root.cloneNode(true) as Element;
  for (const mathml of visible.querySelectorAll(".katex-mathml")) {
    mathml.remove();
  }
  return (visible.textContent ?? "").replace(/\s+/g, " ").trim();
}
