/*
 * The context carrying the reading surface's navigation grammar: the renderer
 * turns a gesture into one of `glance`, `pin`, `go` and the spine decides how
 * each is honored. The grammar runs on internal targets only. An `id:` link
 * carries the id it named; a target built from a note record instead (a relation
 * row, a glossary term) carries an id when the note has one and its slipbox key
 * when it does not.
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

/**
 * How the reader raised a glance. A commit gesture is still ahead of both `hover`
 * and `focus`, but they are told apart because only one of them has a cursor at
 * the link: a card raised by `hover` is read where it is pointed at, one raised
 * by `focus` has to be announced. `touch` is a tap, which nothing follows, so the
 * preview must carry the way onward.
 */
export type GlanceGesture = "hover" | "focus" | "touch";

export interface GlanceRequest {
  readonly target: LinkTarget;
  /** The link element the preview positions itself against. */
  readonly origin: HTMLElement;
  readonly gesture: GlanceGesture;
  /** The `pin` and `go` verbs for this target, each dismissing the preview. */
  readonly pin: () => void;
  readonly go: () => void;
  /** Close the preview, committing to nothing. */
  readonly dismiss: () => void;
  /**
   * The card this request raised is down, dropped by something other than the
   * link that raised it: a scroll carrying the link out from under it, a commit
   * made from the card, another link's card taking its place. A report only, so
   * the raise stops answering for a card that is gone - the drop has happened,
   * and by then another card may already be standing in its place.
   */
  readonly dropped: () => void;
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

/**
 * The target naming a note this surface knows as a record rather than as a link.
 * Prefers an id target, which survives an edit that moves the note, and falls
 * back to the slipbox key for a note carrying no id.
 */
export function targetForNote(key: string, explicitId: string | null): LinkTarget {
  return explicitId
    ? { id: explicitId, target: `id:${explicitId}` }
    : { id: null, target: key };
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
