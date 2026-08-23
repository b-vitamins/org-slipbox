/*
 * The top-level surface mode of the empty frame, mirrored to `?view=`: the search
 * entry (absent parameter), `glossary`, and `review`. Bound to the reading
 * stack's history seam, so the browser and tests drive one reducer.
 */

import { createSignal, type Accessor } from "solid-js";

import { replaceParam } from "./query-url.js";
import { encodeTerm } from "./term-url.js";
import type { StackHistory } from "../reading/stack.js";

const VIEW_PARAM = "view";

/** Which entry surface fills the empty frame. */
export type SurfaceMode = "search" | "glossary" | "review";

/** The modes `view` names. Search is absent: it is the absence of the parameter. */
const NAMED_VIEWS = ["glossary", "review"] as const;

export interface SurfaceView {
  /** The live surface mode. */
  readonly mode: Accessor<SurfaceMode>;
  /** Switch to `mode`, pushing it to the URL. */
  readonly show: (mode: SurfaceMode) => void;
  /** Re-read the mode from the current URL (for `popstate`). */
  readonly sync: () => void;
}

/**
 * Read the surface mode out of a URL's query. Any value the parameter does not
 * name, its absence included, reads as the default search entry.
 */
export function decodeView(url: string): SurfaceMode {
  const query = url.includes("?") ? url.slice(url.indexOf("?") + 1) : "";
  const named = new URLSearchParams(query).get(VIEW_PARAM);
  return NAMED_VIEWS.find((view) => view === named) ?? "search";
}

/**
 * Set `mode` in `url`'s query, keeping every other parameter it carries: `?q=` is
 * owned from the other side and a whole-URL write would drop it. Search removes
 * the parameter, and an emptied query encodes as the bare path.
 *
 * The glossary's open term is the exception: it names a row of a term listing, so
 * the surface that has no listing is not addressed with one. Dropped here, on the
 * URL being pushed, rather than by the glossary as it goes: the entry the reader
 * came from keeps naming the term it was showing, which is what the way back is.
 */
export function encodeView(mode: SurfaceMode, url = ""): string {
  const named = replaceParam(url, VIEW_PARAM, mode === "search" ? null : mode);
  return mode === "search" ? encodeTerm(null, named) : named;
}

/** Create a surface-view store bound to an injectable history. */
export function createSurfaceView(history: StackHistory): SurfaceView {
  const [mode, setMode] = createSignal<SurfaceMode>(decodeView(history.read()));

  return {
    mode,
    show: (next) => {
      setMode(next);
      history.push(encodeView(next, history.read()));
    },
    sync: () => {
      setMode(decodeView(history.read()));
    },
  };
}
