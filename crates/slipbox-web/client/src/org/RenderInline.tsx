/*
 * Render inline Org nodes to DOM.
 *
 * Emphasis and verbatim map to the obvious elements; math defers to KaTeX. A
 * link renders as an anchor whose activation is routed through the navigation
 * seam rather than the browser's default follow, so the spine owns what a click
 * means.
 *
 * An id link's `href` is a genuine destination: the URL the reading stack
 * encodes for opening that target as a fresh root, built by the same
 * `encodeStack` the router reads back. So the link reads as real (hover, visited
 * color) *and* a real browser gesture — new tab, middle-click, copy-link, or a
 * load with no script — lands on the note rather than the landing page. The
 * click handler pre-empts it only to offer the richer in-place path.
 */

import { For, Show, type Component } from "solid-js";

import { encodeStack } from "../reading/stack.js";
import { InlineMath } from "./Math.jsx";
import { useNavigation } from "./navigation.jsx";
import type { Inline } from "./types.js";

const OrgLink: Component<{ node: Extract<Inline, { type: "link" }> }> = (
  props,
) => {
  const navigation = useNavigation();
  const href = (): string =>
    props.node.id ? encodeStack([`id:${props.node.id}`]) : props.node.target;

  const onClick = (event: MouseEvent): void => {
    // Preserve the browser's own gestures for opening elsewhere.
    if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.button !== 0) {
      return;
    }
    event.preventDefault();
    navigation.follow(
      { id: props.node.id, target: props.node.target },
      event.currentTarget as HTMLElement,
    );
  };

  return (
    <a
      class="org-link"
      classList={{ "org-link--external": props.node.id === null }}
      href={href()}
      onClick={onClick}
    >
      <RenderInline nodes={props.node.label} />
    </a>
  );
};

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
