/*
 * The reading surface's navigation seam.
 *
 * A rendered Org link needs to do something when followed, but what it does —
 * push a new column, open a preview — is the spine's concern, not the
 * renderer's. This context is that seam: the renderer calls `follow` with a
 * resolved link target and the surrounding spine decides how to honor it. It is
 * intentionally minimal here (follow only); the glance/pin grammar layers onto
 * the same context in a later milestone.
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

export interface Navigation {
  /** Follow a link — the spine decides how (typically: push a column). */
  readonly follow: (target: LinkTarget, origin: HTMLElement) => void;
}

/** A no-op navigation, so a column renders standalone (e.g. in tests). */
const INERT: Navigation = { follow: () => {} };

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
