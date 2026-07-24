/*
 * The reading surface's navigation grammar.
 *
 * A rendered Org link can be acted on three ways, an escalating ladder of
 * commitment — but *how* each is honored (float a preview, push a column,
 * replace the reading path) is the spine's concern, not the renderer's. This
 * context is that seam: the renderer translates a raw gesture into one grammar
 * verb and the surrounding spine decides what it means.
 *
 * - `glance` — the lightest touch: preview the target without disturbing the
 *   stack or the URL. Passing `null` dismisses the current preview.
 * - `pin`    — open the target as a new column beside its origin (the
 *   stacked-notes default; a plain click or Enter).
 * - `go`     — replace the whole reading path, opening the target as a fresh
 *   root (a deliberate escalation; Alt-click or Alt-Enter).
 *
 * The grammar only ever runs on internal `id:` links; an external link is left
 * to the browser, so every `LinkTarget` reaching a verb carries a real id.
 */

import {
  createContext,
  useContext,
  type Component,
  type JSX,
} from "solid-js";

export interface LinkTarget {
  /** The bare id (uuid) when the link is an `id:` link, else null. */
  readonly id: string | null;
  readonly target: string;
}

/** A request to preview a target, anchored at the link that raised it. */
export interface GlanceRequest {
  readonly target: LinkTarget;
  /** The link element the preview positions itself against. */
  readonly origin: HTMLElement;
}

export interface Navigation {
  /** Preview a target without committing; `null` dismisses the preview. */
  readonly glance: (request: GlanceRequest | null) => void;
  /** Open the target as a new column beside its origin. */
  readonly pin: (target: LinkTarget) => void;
  /** Replace the reading path, opening the target as a fresh root. */
  readonly go: (target: LinkTarget) => void;
}

/** The slipbox reference (an `id:` ref or a raw key) a link target opens. */
export function referenceOf(target: LinkTarget): string {
  return target.id ? `id:${target.id}` : target.target;
}

/** A no-op navigation, so a column renders standalone (e.g. in tests). */
const INERT: Navigation = { glance: () => {}, pin: () => {}, go: () => {} };

const NavigationContext = createContext<Navigation>(INERT);

export const NavigationProvider: Component<{
  navigation: Navigation;
  children: JSX.Element;
}> = (props) => (
  <NavigationContext.Provider value={props.navigation}>
    {props.children}
  </NavigationContext.Provider>
);

/** Read the active navigation; falls back to an inert one when unprovided. */
export function useNavigation(): Navigation {
  return useContext(NavigationContext);
}
