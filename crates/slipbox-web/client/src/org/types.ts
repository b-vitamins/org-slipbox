/*
 * The Org document model the reading column renders: id links, inline and display
 * math, `/italic/`, `*bold*`, `=verbatim=`, headings, plain and ordered lists,
 * source, example and quote blocks, and tables. Frontmatter (`:PROPERTIES:`,
 * `#+KEYWORD:`, comments) is consumed by the parser and absent from the model.
 */

export type Inline =
  | { readonly type: "text"; readonly value: string }
  | { readonly type: "bold"; readonly children: readonly Inline[] }
  | { readonly type: "italic"; readonly children: readonly Inline[] }
  /** Literal verbatim (`=...=`), which in Org never contains nested markup. */
  | { readonly type: "verbatim"; readonly value: string }
  /** Inline math (`\(...\)`), carrying TeX normalized for KaTeX. */
  | { readonly type: "math"; readonly tex: string }
  /**
   * An Org link. `target` is the raw link target (for id links, `id:<uuid>`);
   * `id` is the bare uuid when the target is an id link, else null. The label is
   * parsed inline, since an Org link description may carry math or emphasis.
   */
  | {
      readonly type: "link";
      readonly target: string;
      readonly id: string | null;
      readonly label: readonly Inline[];
    };

/**
 * One item of a list. `children` is the item's marker line plus the lines the
 * source wraps beneath it, joined into one run. `blocks` holds the constructs
 * nested under the item, which Org marks purely by indent.
 */
export interface ListItem {
  readonly children: readonly Inline[];
  readonly blocks: readonly Block[];
}

export interface TableCell {
  readonly children: readonly Inline[];
}

/**
 * One row of a table. `header` marks a row above the table's `|--|` rule line,
 * which is Org's header convention, so the renderer can emit `th`.
 */
export interface TableRow {
  readonly header: boolean;
  readonly cells: readonly TableCell[];
}

/**
 * A list. `start` is the number the source's first item carries, so an Org list
 * resuming after an interruption continues its numbering.
 */
export interface ListBlock {
  readonly type: "list";
  readonly ordered: boolean;
  readonly start: number;
  readonly items: readonly ListItem[];
}

export type Block =
  | {
      readonly type: "heading";
      readonly level: number;
      readonly children: readonly Inline[];
    }
  | { readonly type: "paragraph"; readonly children: readonly Inline[] }
  | ListBlock
  | {
      readonly type: "src";
      readonly lang: string | null;
      readonly code: string;
    }
  | { readonly type: "example"; readonly text: string }
  /** A `#+begin_quote` block, which in Org holds blocks rather than a run. */
  | { readonly type: "quote"; readonly blocks: readonly Block[] }
  /** Display math (`\[...\]`), carrying TeX normalized for KaTeX. */
  | { readonly type: "math"; readonly tex: string }
  | { readonly type: "table"; readonly rows: readonly TableRow[] };

export interface OrgDocument {
  readonly blocks: readonly Block[];
}
