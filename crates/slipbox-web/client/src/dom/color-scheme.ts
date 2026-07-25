/*
 * The reader's color scheme, persisted in `localStorage` and applied as a root
 * attribute the palette tokens key `color-scheme` off.
 *
 * Storage access is guarded: a browser configured to block it throws on the
 * property itself. `index.html` repeats the read before first paint so a stored
 * scheme opposing the platform's does not flash the wrong canvas.
 */

import { createSignal, type Accessor } from "solid-js";

/** What the reader has asked for: the platform's scheme, or one of the two. */
export type ColorScheme = "system" | "light" | "dark";

/** Where the reader's choice persists. */
export const SCHEME_STORAGE_KEY = "slipbox:color-scheme";

/** The root attribute the palette tokens key `color-scheme` off. */
export const SCHEME_ATTRIBUTE = "data-theme";

/** The cycle one press of the control walks, starting from the platform. */
export const SCHEME_ORDER: readonly ColorScheme[] = ["system", "light", "dark"];

/** The reader-facing name of each scheme. */
export const SCHEME_LABELS: Readonly<Record<ColorScheme, string>> = {
  system: "Auto",
  light: "Light",
  dark: "Dark",
};

export interface ColorSchemeStore {
  readonly scheme: Accessor<ColorScheme>;
  /** Choose `scheme`, applying and persisting it. */
  readonly select: (scheme: ColorScheme) => void;
  readonly cycle: () => void;
}

/**
 * Read a scheme out of a stored string. Anything unrecognized, an absent key
 * included, is the platform default.
 */
export function decodeScheme(raw: string | null | undefined): ColorScheme {
  return raw === "light" || raw === "dark" ? raw : "system";
}

/** The scheme one press after `current`, wrapping around the cycle. */
export function nextScheme(current: ColorScheme): ColorScheme {
  const index = SCHEME_ORDER.indexOf(current);
  return SCHEME_ORDER[(index + 1) % SCHEME_ORDER.length] ?? "system";
}

/** The persisted scheme, or the platform default when storage is unreachable. */
export function readStoredScheme(): ColorScheme {
  try {
    return decodeScheme(window.localStorage.getItem(SCHEME_STORAGE_KEY));
  } catch {
    return "system";
  }
}

/** Persist `scheme`, removing the key entirely for the platform default. */
export function storeScheme(scheme: ColorScheme): void {
  try {
    if (scheme === "system") {
      window.localStorage.removeItem(SCHEME_STORAGE_KEY);
    } else {
      window.localStorage.setItem(SCHEME_STORAGE_KEY, scheme);
    }
  } catch {
    // Storage blocked: the applied attribute still governs this page.
  }
}

/**
 * Apply `scheme` to the document root. The platform default is the absence of
 * the attribute, so the tokens' own `color-scheme: light dark` governs.
 */
export function applyScheme(scheme: ColorScheme): void {
  if (typeof document === "undefined") {
    return;
  }
  if (scheme === "system") {
    document.documentElement.removeAttribute(SCHEME_ATTRIBUTE);
  } else {
    document.documentElement.setAttribute(SCHEME_ATTRIBUTE, scheme);
  }
}

/** Create a color-scheme store seeded from what the reader last chose. */
export function createColorScheme(): ColorSchemeStore {
  const [scheme, setScheme] = createSignal<ColorScheme>(readStoredScheme());

  const select = (next: ColorScheme): void => {
    setScheme(next);
    applyScheme(next);
    storeScheme(next);
  };

  return { scheme, select, cycle: () => select(nextScheme(scheme())) };
}
