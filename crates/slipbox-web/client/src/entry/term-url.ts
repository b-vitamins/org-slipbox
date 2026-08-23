import { replaceParam } from "./query-url.js";

const TERM_PARAM = "term";

export interface TermUrl {
  readonly read: () => string | null;
  readonly replace: (key: string | null) => void;
}

export function decodeTerm(search: string): string | null {
  const query = search.startsWith("?") ? search.slice(1) : search;
  const value = new URLSearchParams(query).get(TERM_PARAM)?.trim();
  return value === undefined || value === "" ? null : value;
}

export function encodeTerm(key: string | null, url: string): string {
  return replaceParam(url, TERM_PARAM, key?.trim() || null);
}

export function browserTermUrl(): TermUrl {
  return {
    read: () => decodeTerm(window.location.search),
    replace: (key) => {
      const url = `${window.location.pathname}${window.location.search}`;
      window.history.replaceState(
        window.history.state,
        "",
        encodeTerm(key, url),
      );
    },
  };
}
