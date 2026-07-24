/*
 * Render inline Org nodes to DOM.
 *
 * Emphasis and verbatim map to the obvious elements; math defers to KaTeX. An
 * internal `id:` link is the interactive heart of the surface: it renders as an
 * anchor whose gestures are translated into the navigation grammar rather than
 * a browser follow — hover or focus `glance`s a preview, a plain click `pin`s a
 * column, and an Alt-click `go`es to a fresh root. Keyboard activation needs no
 * separate handler: a focused anchor activates on Enter, synthesizing a click
 * that flows through the same `onClick` and pins the target. An external link
 * (no id) carries no grammar at all — the browser follows it as usual.
 *
 * The `href` is a genuine destination, not a decorative fake: it is the very URL
 * the reading stack encodes for opening this target as a fresh root, built by
 * the same `encodeStack` the router reads back. So a real browser gesture — new
 * tab, command-click, copy-link, or a no-JS load — lands on the note rather than
 * the landing page, and the grammar is only a richer path over the same address.
 */

import { For, Show, type Component } from "solid-js";

import { encodeStack } from "../reading/stack.js";
import { InlineMath } from "./Math.jsx";
import { referenceOf, useNavigation, type LinkTarget } from "./navigation.jsx";
import type { Inline } from "./types.js";

/** True for a click the browser should keep (new tab, non-primary button). */
function isBrowserGesture(event: MouseEvent): boolean {
  return (
    event.defaultPrevented ||
    event.metaKey ||
    event.ctrlKey ||
    event.button !== 0
  );
}

const OrgLink: Component<{ node: Extract<Inline, { type: "link" }> }> = (
  props,
) => {
  const navigation = useNavigation();
  const isInternal = (): boolean => props.node.id !== null;
  const target = (): LinkTarget => ({
    id: props.node.id,
    target: props.node.target,
  });
  const href = (): string =>
    isInternal() ? encodeStack([referenceOf(target())]) : props.node.target;

  // An Alt gesture escalates to `go` (replace the path); otherwise `pin`.
  const commit = (element: HTMLElement, alt: boolean): void => {
    navigation.glance(null);
    if (alt) {
      navigation.go(target());
    } else {
      navigation.pin(target());
    }
    // Drop focus so the just-followed link does not re-`glance` itself.
    element.blur();
  };

  const onClick = (event: MouseEvent): void => {
    if (!isInternal() || isBrowserGesture(event)) {
      return;
    }
    event.preventDefault();
    commit(event.currentTarget as HTMLElement, event.altKey);
  };

  const glance = (event: FocusEvent | MouseEvent): void => {
    if (isInternal()) {
      navigation.glance({
        target: target(),
        origin: event.currentTarget as HTMLElement,
      });
    }
  };

  const dismiss = (): void => {
    if (isInternal()) {
      navigation.glance(null);
    }
  };

  return (
    <a
      class="org-link"
      classList={{ "org-link--external": !isInternal() }}
      href={href()}
      onClick={onClick}
      onMouseEnter={glance}
      onMouseLeave={dismiss}
      onFocus={glance}
      onBlur={dismiss}
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
