/*
 * Render inline Org nodes to DOM. Source-aware Org links render through
 * `GrammarLink`, which routes gestures to the navigation grammar. Safe external
 * links retain normal browser behavior unless an embedding host intercepts them.
 */

import {
  createEffect,
  createMemo,
  createSignal,
  For,
  Show,
  type Component,
} from "solid-js";

import { useAssetResolver } from "./assets.jsx";
import { GrammarLink, isBrowserGesture } from "./GrammarLink.jsx";
import {
  followableHref,
  isOrgDocumentTarget,
  resolvedAssetHref,
} from "./link-target.js";
import { useNavigation } from "./navigation.jsx";
import { InlineMath } from "./Math.jsx";
import type { Inline } from "./types.js";

import "./org.css";

type LinkNode = Extract<Inline, { type: "link" }>;

const IMAGE_TARGET = /\.(?:png|jpe?g|gif|webp)$/i;

function plainText(nodes: readonly Inline[]): string {
  return nodes
    .map((node) => {
      switch (node.type) {
        case "text":
        case "verbatim":
          return node.value;
        case "bold":
        case "italic":
          return plainText(node.children);
        case "math":
          return node.tex;
        case "link":
          return plainText(node.label);
        default:
          return "";
      }
    })
    .join("")
    .trim();
}

function bareImage(node: LinkNode): boolean {
  return (
    IMAGE_TARGET.test(node.target.split("::", 1)[0] ?? "") &&
    node.label.length === 1 &&
    node.label[0]?.type === "text" &&
    node.label[0].value === node.target
  );
}

function imageLabel(node: LinkNode): string {
  const described = plainText(node.label);
  if (described !== node.target && described !== "") {
    return described;
  }
  const path = node.target.replace(/^file:/i, "").split("::", 1)[0] ?? "";
  return path.split(/[\\/]/).pop() || "Repository image";
}

const ResolvedImage: Component<{
  node: LinkNode;
  href: string;
  follow: (event: MouseEvent) => void;
}> = (props) => {
  const [failed, setFailed] = createSignal(false);
  createEffect(() => {
    props.href;
    setFailed(false);
  });
  const label = (): string => imageLabel(props.node);
  return (
    <Show
      when={!failed()}
      fallback={
        <span
          class="org-image org-image--unavailable"
          role="img"
          aria-label={`Image unavailable: ${label()}`}
        >
          <span class="org-image__state">Image unavailable</span>
          <span class="org-image__label">{label()}</span>
        </span>
      }
    >
      <span class="org-image">
        <a
          class="org-image__link"
          href={props.href}
          aria-label={`Open image: ${label()}`}
          onClick={props.follow}
        >
          <img
            class="org-image__content"
            src={props.href}
            alt={label()}
            loading="lazy"
            decoding="async"
            onError={() => setFailed(true)}
          />
        </a>
      </span>
    </Show>
  );
};

/**
 * Render a target the document cannot follow itself: as an anchor where the host
 * resolves it to a URL of an admitted scheme, and otherwise as readable inert text.
 */
const ExternalLink: Component<{ node: LinkNode }> = (props) => {
  const resolveAsset = useAssetResolver();
  const navigation = useNavigation();
  const asset = createMemo((): string | null => {
    const resolved = resolveAsset(props.node.target);
    return resolved === null ? null : resolvedAssetHref(resolved);
  });
  const follow = (event: MouseEvent): void => {
    if (isBrowserGesture(event)) {
      return;
    }
    if (navigation.external?.({ id: null, target: props.node.target }) === true) {
      event.preventDefault();
    }
  };
  const image = (): string | null => (bareImage(props.node) ? asset() : null);
  return (
    <Show
      when={followableHref(props.node.target)}
      fallback={
        <Show
          when={asset()}
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
            <Show
              when={image()}
              fallback={
                <a class="org-link org-link--asset" href={href()} onClick={follow}>
                  <RenderInline nodes={props.node.label} />
                </a>
              }
            >
              {(imageHref) => (
                <ResolvedImage node={props.node} href={imageHref()} follow={follow} />
              )}
            </Show>
          )}
        </Show>
      }
    >
      {(href) => (
        <a class="org-link org-link--external" href={href()} onClick={follow}>
          <RenderInline nodes={props.node.label} />
        </a>
      )}
    </Show>
  );
};

const OrgLink: Component<{ node: LinkNode }> = (props) => (
  <Show
    when={props.node.id !== null || isOrgDocumentTarget(props.node.target)}
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
