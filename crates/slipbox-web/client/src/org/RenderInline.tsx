/*
 * Render inline Org nodes to DOM. An internal `id:` link renders through
 * `GrammarLink`, which routes gestures to the navigation grammar; an external
 * safe external link carries no grammar and is followed by the browser.
 */

import { For, Show, type Component } from "solid-js";

import { GrammarLink } from "./GrammarLink.jsx";
import { followableHref } from "./link-target.js";
import { InlineMath } from "./Math.jsx";
import type { Inline } from "./types.js";

type LinkNode = Extract<Inline, { type: "link" }>;

/** Render unsupported external targets as readable inert text. */
const ExternalLink: Component<{ node: LinkNode }> = (props) => (
  <Show
    when={followableHref(props.node.target)}
    fallback={
      <span
        class="org-link org-link--inert"
        title="This link's target is not one the reading surface follows."
      >
        <RenderInline nodes={props.node.label} />
      </span>
    }
  >
    {(href) => (
      <a class="org-link org-link--external" href={href()}>
        <RenderInline nodes={props.node.label} />
      </a>
    )}
  </Show>
);

const OrgLink: Component<{ node: LinkNode }> = (props) => (
  <Show
    when={props.node.id !== null}
    fallback={<ExternalLink node={props.node} />}
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
