/*
 * The entry surface's search term, mirrored to the URL's `?q=`. The reading stack
 * owns `?note=&stacked=` and pushes per navigation; the term replaces the current
 * entry instead. One query, one owner per parameter (the term here, `?view=` in
 * `surface-view.ts`), so both write through `replaceParam`.
 */

const QUERY_PARAM = "q";

/** The persistence seam: read the current `?q=` and replace it in place. */
export interface QueryUrl {
  /** The URL's search term, trimmed, or `null` when it names none. */
  readonly read: () => string | null;
  /** Replace the URL's term in place (no new history entry); `null` clears it. */
  readonly replace: (term: string | null) => void;
}

/**
 * Parse a location search string into the search term, or `null` when it names
 * none. The value is trimmed, so whitespace alone (`?q=%20`) is no term.
 */
export function decodeQuery(search: string): string | null {
  const query = search.startsWith("?") ? search.slice(1) : search;
  const value = new URLSearchParams(query).get(QUERY_PARAM)?.trim();
  return value === undefined || value === "" ? null : value;
}

/**
 * Set `name` to `value` inside `url`'s query, keeping its path and every other
 * parameter it carries; a `null` value removes the parameter. An emptied query
 * encodes as the bare path rather than trailing a lone `?`.
 */
export function replaceParam(
  url: string,
  name: string,
  value: string | null,
): string {
  const mark = url.indexOf("?");
  const path = mark === -1 ? url : url.slice(0, mark);
  const params = new URLSearchParams(mark === -1 ? "" : url.slice(mark + 1));
  if (value === null) {
    params.delete(name);
  } else {
    params.set(name, value);
  }
  const query = params.toString();
  return query === "" ? path : `${path}?${query}`;
}

/**
 * Set `term` as `?q=` within `url`, keeping the rest of the query. An empty term
 * removes the parameter.
 */
export function encodeQuery(term: string | null, url: string): string {
  return replaceParam(url, QUERY_PARAM, term?.trim() || null);
}

/** A `QueryUrl` backed by `window.location` and `history.replaceState`. */
export function browserQueryUrl(): QueryUrl {
  return {
    read: () => decodeQuery(window.location.search),
    // Replace, never push: the entry surface occupies one history slot. Both the
    // whole address and the entry's existing state go back in, because
    // `replaceState` rewrites an entry whole: the address carries `?view=` and the
    // state carries the result cursor (`cursor-history.ts`), and either omitted
    // here is either unset.
    replace: (term) => {
      const url = `${window.location.pathname}${window.location.search}`;
      window.history.replaceState(
        window.history.state,
        "",
        encodeQuery(term, url),
      );
    },
  };
}
