/*
 * The standalone document bundle's public contract.
 *
 * A host supplies Org source, a scheme, and optional hooks; the bundle renders
 * it and reports gestures. It owns no URL grammar, no history, no storage and no
 * transport: navigation is the host's, and every hook is the host's own function,
 * never something the rendered content supplies.
 */

/** The palette scheme a mounted document paints in. */
export type OrgDocumentTheme = "system" | "light" | "dark";

/** How a preview gesture was raised. */
export type OrgDocumentGesture = "hover" | "focus" | "touch";

export interface OrgDocumentLink {
  /** The bare uuid when the link is an `id:` link, else null. */
  readonly id: string | null;
  /** The link target verbatim from the Org source. */
  readonly target: string;
  /** What a host resolves: `id:<uuid>` where there is an id, else the target. */
  readonly reference: string;
}

/**
 * One navigation gesture, reported and not acted on. `glance` is a preview
 * request the host may draw against `origin`; `dismiss` withdraws the standing
 * one and names no link.
 */
export type OrgDocumentIntent =
  | {
      readonly verb: "glance";
      readonly link: OrgDocumentLink;
      readonly origin: HTMLElement;
      readonly gesture: OrgDocumentGesture;
    }
  | { readonly verb: "pin" | "go"; readonly link: OrgDocumentLink }
  | { readonly verb: "dismiss" };

export interface OrgDocumentContent {
  /** Org source, parsed as data; nothing in it is ever evaluated. */
  readonly source: string;
  /** The level the source's top headings render at, `h2` by default. */
  readonly baseLevel?: number;
}

export interface OrgDocumentOptions {
  readonly content: OrgDocumentContent;
  /** Defaults to `system`, which follows the platform. */
  readonly theme?: OrgDocumentTheme;
  /**
   * Receives every gesture; absent means gestures are dropped. A standing
   * `glance` is closed by a `dismiss` to the hook that raised it before the nodes
   * it points at are replaced, before this hook itself is replaced, and on
   * disposal. A hook the host has replaced receives nothing further.
   */
  readonly onIntent?: (intent: OrgDocumentIntent) => void;
  /**
   * The URL an internal link carries. Returning null leaves the anchor without
   * one, which is also what an absent hook does.
   */
  readonly href?: (link: OrgDocumentLink) => string | null;
  /**
   * Resolve a target the renderer cannot follow (`file:`, `attachment:`) to a URL
   * for it. Relative URLs and the `http`, `https` and `blob` schemes are admitted
   * and anything else is left inert; the document fetches none of them itself.
   */
  readonly resolveAsset?: (target: string) => string | null;
}

export interface OrgDocumentHandle {
  /**
   * Replace the named options. A key that is present replaces its value, an
   * absent one keeps what is mounted, and `content` cannot be dropped. New source
   * or base level renders new nodes; any other change keeps the mounted ones, so
   * an element the host still holds stays live and answers to whichever hooks and
   * asset policy were supplied last.
   */
  readonly update: (options: Partial<OrgDocumentOptions>) => void;
  /**
   * Unmount: removes the rendered nodes, releases the handlers and timers the
   * document holds, and leaves the host element's own children untouched.
   * Calling it twice is harmless. Controls and links the host kept report nothing
   * further and start no clipboard write, and one already in flight settles
   * unobserved; an anchor's own browser behavior remains the browser's.
   */
  readonly dispose: () => void;
}
