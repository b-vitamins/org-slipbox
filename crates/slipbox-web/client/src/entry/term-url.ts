/*
 * The glossary's open term, mirrored to the URL's `?term=`. One parameter, one
 * owner: the entry surface's search term is `?q=` (`query-url.ts`), its mode is
 * `?view=` (`surface-view.ts`), and the reading stack owns `?note=&stacked=`.
 *
 * The value is a slipbox key, which is what a term listing names its rows by and
 * what `/api/glossary/term` resolves. A reference (`id:<uuid>`) is the reading
 * surface's currency instead, where a note reached from anywhere has to be namable
 * without knowing which file it landed in.
 *
 * Peeking a term costs no history entry, so a selection replaces the current entry
 * rather than pushing one, the same rule `?q=` is written under.
 */

import { replaceParam } from "./query-url.js";

const TERM_PARAM = "term";

/** The persistence seam: read the current `?term=` and replace it in place. */
export interface TermUrl {
  /** The key of the open term, or `null` when the address names none. */
  readonly read: () => string | null;
  /** Replace the open term in place (no new history entry); `null` clears it. */
  readonly replace: (key: string | null) => void;
}

/**
 * The key `?term=` names in a location search string, or `null` when it names
 * none. Trimmed, so whitespace alone is no key.
 */
export function decodeTerm(search: string): string | null {
  const query = search.startsWith("?") ? search.slice(1) : search;
  const value = new URLSearchParams(query).get(TERM_PARAM)?.trim();
  return value === undefined || value === "" ? null : value;
}

/**
 * Set `key` as `?term=` within `url`, keeping every other parameter it carries. A
 * null or blank key removes the parameter, and an emptied query encodes as the
 * bare path.
 */
export function encodeTerm(key: string | null, url: string): string {
  return replaceParam(url, TERM_PARAM, key?.trim() || null);
}

/** A `TermUrl` backed by `window.location` and `history.replaceState`. */
export function browserTermUrl(): TermUrl {
  return {
    read: () => decodeTerm(window.location.search),
    // The whole address and the entry's existing state both go back in, because
    // `replaceState` rewrites an entry whole: either omitted here is either unset.
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
