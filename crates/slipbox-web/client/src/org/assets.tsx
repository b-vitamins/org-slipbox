import { createContext, useContext, type Component, type JSX } from "solid-js";

/**
 * Resolve an unsupported target to a scheme-checked URL, or null to keep it inert.
 * HTTP(S) results are allowed; the renderer does not enforce local origins.
 */
export type AssetResolver = (target: string) => string | null;

const RESOLVES_NOTHING: AssetResolver = () => null;

const AssetContext = createContext<AssetResolver>(RESOLVES_NOTHING);

export const AssetProvider: Component<{
  resolve: AssetResolver;
  children: JSX.Element;
}> = (props) => (
  <AssetContext.Provider value={props.resolve}>
    {props.children}
  </AssetContext.Provider>
);

export function useAssetResolver(): AssetResolver {
  return useContext(AssetContext);
}
