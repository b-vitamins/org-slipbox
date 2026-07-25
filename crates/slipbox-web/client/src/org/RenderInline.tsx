/*
 * Render inline Org nodes to DOM. An internal `id:` link renders through
 * `GrammarLink`, which routes gestures to the navigation grammar; an external
 * link carries no grammar and is a plain anchor the browser follows.
 */

import { For, Show, type Component } from "solid-js";

import { GrammarLink } from "./GrammarLink.jsx";
import { InlineMath } from "./Math.jsx";
import type { Inline } from "./types.js";

const OrgLink: Component<{ node: Extract<Inline, { type: "link" }> }> = (
  props,
) => (
  <Show
    when={props.node.id !== null}
    fallback={
      <a class="org-link org-link--external" href={props.node.target}>
        <RenderInline nodes={props.node.label} />
      </a>
    }
  >
    <GrammarLink
      class="org-link"
      target={{ id: props.node.id, target: props.node.target }}
    >
      <RenderInline nodes={props.node.label} />
    </GrammarLink>
  </Show>
);

export const RenderInline: Component<{ nodes: readonly Inline[] }> = (props) => (
  <For each={props.nodes}>
    {(node) => (
      <Show when={node.type === "text" && node} fallback={<InlineNonText node={node} />}>
        {(text) => <>{text().value}</>}
      </Show>
    )}
  </For>
);

const InlineNonText: Component<{ node: Inline }> = (props) => {
  const node = props.node;
  switch (node.type) {
    case "bold":
      return (
        <strong>
          <RenderInline nodes={node.children} />
        </strong>
      );
    case "italic":
      return (
        <em>
          <RenderInline nodes={node.children} />
        </em>
      );
    case "verbatim":
      return <code class="org-verbatim">{node.value}</code>;
    case "math":
      return <InlineMath tex={node.tex} />;
    case "link":
      return <OrgLink node={node} />;
    default:
      return null;
  }
};
