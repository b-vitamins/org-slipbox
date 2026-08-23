/*
 * Render a parsed Org document to DOM, one semantic element per block.
 *
 * Body headings start below the surface heading, at `h2` by default.
 */

import { For, Show, type Component } from "solid-js";
import { Dynamic } from "solid-js/web";

import { DisplayMath } from "./Math.jsx";
import { RenderInline } from "./RenderInline.jsx";
import { SourceBlock } from "./SourceBlock.jsx";
import type { Block, ListBlock, ListItem, OrgDocument } from "./types.js";

type HeadingTag = "h2" | "h3" | "h4" | "h5" | "h6";

const DEFAULT_BASE_LEVEL = 2;

function headingTag(level: number, base: number): HeadingTag {
  const clamped = Math.min(Math.max(level + base - 1, base), 6);
  return `h${clamped}` as HeadingTag;
}

const RenderItem: Component<{ item: ListItem; base: number }> = (props) => (
  <li>
    <RenderInline nodes={props.item.children} />
    <For each={props.item.blocks}>
      {(block) => <RenderBlock block={block} base={props.base} />}
    </For>
  </li>
);

/** An ordered list carries `start`, so an interrupted Org list keeps counting. */
const RenderList: Component<{ list: ListBlock; base: number }> = (props) => (
  <Show
    when={props.list.ordered}
    fallback={
      <ul class="org-list">
        <For each={props.list.items}>
          {(item) => <RenderItem item={item} base={props.base} />}
        </For>
      </ul>
    }
  >
    <ol class="org-list" start={props.list.start}>
      <For each={props.list.items}>
        {(item) => <RenderItem item={item} base={props.base} />}
      </For>
    </ol>
  </Show>
);

const RenderBlock: Component<{ block: Block; base: number }> = (props) => {
  const block = props.block;
  switch (block.type) {
    case "heading":
      return (
        <Dynamic
          component={headingTag(block.level, props.base)}
          class="org-heading"
        >
          <RenderInline nodes={block.children} />
        </Dynamic>
      );
    case "paragraph":
      return (
        <p class="org-paragraph">
          <RenderInline nodes={block.children} />
        </p>
      );
    case "list":
      return <RenderList list={block} base={props.base} />;
    case "src":
      return <SourceBlock lang={block.lang} code={block.code} />;
    case "example":
      return (
        <pre class="org-example">
          <code>{block.text}</code>
        </pre>
      );
    case "quote":
      return (
        <blockquote class="org-quote">
          <For each={block.blocks}>
            {(nested) => <RenderBlock block={nested} base={props.base} />}
          </For>
        </blockquote>
      );
    case "math":
      return <DisplayMath tex={block.tex} />;
    case "table": {
      const headerRows = block.rows.filter((row) => row.header);
      const bodyRows = block.rows.filter((row) => !row.header);
      return (
        <table class="org-table">
          <Show when={headerRows.length > 0}>
            <thead>
              <For each={headerRows}>
                {(row) => (
                  <tr>
                    <For each={row.cells}>
                      {(cell) => (
                        <th scope="col">
                          <RenderInline nodes={cell.children} />
                        </th>
                      )}
                    </For>
                  </tr>
                )}
              </For>
            </thead>
          </Show>
          <tbody>
            <For each={bodyRows}>
              {(row) => (
                <tr>
                  <For each={row.cells}>
                    {(cell) => (
                      <td>
                        <RenderInline nodes={cell.children} />
                      </td>
                    )}
                  </For>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      );
    }
    default:
      return null;
  }
};

export const RenderDocument: Component<{
  document: OrgDocument;
  baseLevel?: number;
}> = (props) => (
  <div class="org-document">
    <For each={props.document.blocks}>
      {(block) => (
        <RenderBlock block={block} base={props.baseLevel ?? DEFAULT_BASE_LEVEL} />
      )}
    </For>
  </div>
);
