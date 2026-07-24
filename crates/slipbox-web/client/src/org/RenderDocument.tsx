/*
 * Render a parsed Org document to DOM, one semantic element per block.
 *
 * Body headings start at `h2`: the note's title is the `h1` the column supplies,
 * so a first-level Org heading is a section within the note, not its peer.
 */

import { For, Show, type Component } from "solid-js";
import { Dynamic } from "solid-js/web";

import { DisplayMath } from "./Math.jsx";
import { RenderInline } from "./RenderInline.jsx";
import { SourceBlock } from "./SourceBlock.jsx";
import type { Block, ListBlock, ListItem, OrgDocument } from "./types.js";

/** Clamp an Org heading level to the `h2` to `h6` range used within a note. */
function headingTag(level: number): "h2" | "h3" | "h4" | "h5" | "h6" {
  const clamped = Math.min(Math.max(level + 1, 2), 6);
  return `h${clamped}` as "h2" | "h3" | "h4" | "h5" | "h6";
}

const RenderItem: Component<{ item: ListItem }> = (props) => (
  <li>
    <RenderInline nodes={props.item.children} />
    <For each={props.item.blocks}>{(block) => <RenderBlock block={block} />}</For>
  </li>
);

/** An ordered list carries `start`, so an interrupted Org list keeps counting. */
const RenderList: Component<{ list: ListBlock }> = (props) => (
  <Show
    when={props.list.ordered}
    fallback={
      <ul class="org-list">
        <For each={props.list.items}>{(item) => <RenderItem item={item} />}</For>
      </ul>
    }
  >
    <ol class="org-list" start={props.list.start}>
      <For each={props.list.items}>{(item) => <RenderItem item={item} />}</For>
    </ol>
  </Show>
);

const RenderBlock: Component<{ block: Block }> = (props) => {
  const block = props.block;
  switch (block.type) {
    case "heading":
      return (
        <Dynamic component={headingTag(block.level)} class="org-heading">
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
      return <RenderList list={block} />;
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
          <For each={block.blocks}>{(nested) => <RenderBlock block={nested} />}</For>
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

export const RenderDocument: Component<{ document: OrgDocument }> = (props) => (
  <div class="org-document">
    <For each={props.document.blocks}>{(block) => <RenderBlock block={block} />}</For>
  </div>
);
