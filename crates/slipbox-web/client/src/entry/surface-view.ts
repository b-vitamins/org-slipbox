import { createSignal, type Accessor } from "solid-js";

import { decodeQuery, encodeQuery, replaceParam } from "./query-url.js";
import { encodeTerm } from "./term-url.js";
import type { StackHistory } from "../reading/stack.js";

const VIEW_PARAM = "view";

export type SurfaceMode = "search" | "glossary" | "review";

const NAMED_VIEWS = ["glossary", "review"] as const;

export interface SurfaceView {
  readonly mode: Accessor<SurfaceMode>;
  readonly show: (mode: SurfaceMode) => void;
  readonly sync: () => void;
}

export function decodeView(url: string): SurfaceMode {
  const query = url.includes("?") ? url.slice(url.indexOf("?") + 1) : "";
  const named = new URLSearchParams(query).get(VIEW_PARAM);
  return NAMED_VIEWS.find((view) => view === named) ?? "search";
}

/** Encode a surface switch while preserving unrelated query parameters. */
export function encodeView(mode: SurfaceMode, url = ""): string {
  const named = replaceParam(url, VIEW_PARAM, mode === "search" ? null : mode);
  return mode === "search" ? encodeTerm(null, named) : named;
}

export function createSurfaceView(history: StackHistory): SurfaceView {
  const initial = decodeView(history.read());
  const [mode, setMode] = createSignal<SurfaceMode>(initial);
  const queries: Record<"search" | "glossary", string | null> = {
    search: null,
    glossary: null,
  };
  const family = (value: SurfaceMode): "search" | "glossary" =>
    value === "search" ? "search" : "glossary";
  queries[family(initial)] = decodeQuery(history.read());

  return {
    mode,
    show: (next) => {
      queries[family(mode())] = decodeQuery(history.read());
      const address = encodeQuery(
        queries[family(next)],
        history.read(),
      );
      history.push(encodeView(next, address));
      setMode(next);
    },
    sync: () => {
      const next = decodeView(history.read());
      queries[family(next)] = decodeQuery(history.read());
      setMode(next);
    },
  };
}
