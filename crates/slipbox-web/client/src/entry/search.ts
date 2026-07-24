/*
 * The debounced search controller: a raw `query` signal the input shows and a
 * debounced `term` signal that keys the resource. `term` is `null` for an empty field
 * or one holding no word the index can match, which is the falsy value a reading
 * resource defers on, so the `q`-is-required search never fires empty.
 */

import { createSignal, type Accessor } from "solid-js";

import { WINDOW_SCHEDULER, type Scheduler } from "../data/scheduler.js";
import type { QueryUrl } from "./query-url.js";

/**
 * The shortest word, in characters (not bytes), a search term can be built from.
 *
 * Restates `MIN_SEARCH_TERM_CHARACTERS` in `crates/slipbox-core/src/nodes.rs`,
 * which the server enforces; `search.test.ts` reads that Rust source back to hold
 * the two equal.
 */
export const MIN_TERM_CHARACTERS = 2;

/** Alphanumeric in the sense Rust's `char::is_alphanumeric` means it. */
const ALPHANUMERIC = /[\p{Alphabetic}\p{Number}]/u;

/** Whether `raw` holds a word the index can match, by the server's rule. */
function isSearchable(raw: string): boolean {
  return raw
    .split(/\s+/)
    .some(
      (word) =>
        // `Array.from` counts code points, the unit the server's `chars()` counts.
        // A plain `.length` counts UTF-16 units, so an astral character would
        // count as the two surrogates it is stored as.
        Array.from(trimToAlphanumeric(word)).length >= MIN_TERM_CHARACTERS,
    );
}

/** `word` without its leading and trailing non-alphanumeric characters. */
function trimToAlphanumeric(word: string): string {
  const characters = [...word];
  let start = 0;
  let end = characters.length;
  while (start < end && !ALPHANUMERIC.test(characters[start]!)) {
    start += 1;
  }
  while (end > start && !ALPHANUMERIC.test(characters[end - 1]!)) {
    end -= 1;
  }
  return characters.slice(start, end).join("");
}

/** The term `raw` settles into, or `null` when it holds nothing searchable. */
function settledTerm(raw: string): string | null {
  const trimmed = raw.trim();
  return trimmed !== "" && isSearchable(trimmed) ? trimmed : null;
}

/** A `QueryUrl` that neither reads nor persists: the default off-URL controller. */
const NULL_QUERY_URL: QueryUrl = { read: () => null, replace: () => {} };

export interface SearchController {
  /** The raw field text, updated on every keystroke. */
  readonly query: Accessor<string>;
  /** The debounced, trimmed search term, or `null` when the field is empty. */
  readonly term: Accessor<string | null>;
  /**
   * Whether the field shows a query the term does not: a keystroke inside the
   * debounce window, whose results have not been asked for yet. What a surface
   * lists answers `term`, not the field.
   */
  readonly pending: Accessor<boolean>;
  /** Whether the field holds text but no word long enough to search on. */
  readonly awaitingWord: Accessor<boolean>;
  /** Record a keystroke: show `value` now, settle it into `term` after intent. */
  readonly input: (value: string) => void;
  /** Settle the field's current text into the term now, skipping the debounce. */
  readonly settle: () => void;
  /** Clear the field and the term at once (no debounce). */
  readonly clear: () => void;
  /** Cancel any pending term update, for owner cleanup. */
  readonly cancel: () => void;
}

/**
 * Create a search controller with a `debounce` window (ms) before a term settles.
 * A non-positive window settles at once. The optional `queryUrl` both seeds the
 * initial term and receives every settled one.
 */
export function createSearchController(
  debounce = 180,
  scheduler: Scheduler = WINDOW_SCHEDULER,
  queryUrl: QueryUrl = NULL_QUERY_URL,
): SearchController {
  const initial = queryUrl.read();
  const [query, setQuery] = createSignal(initial ?? "");
  const [term, setTerm] = createSignal<string | null>(
    initial === null ? null : settledTerm(initial),
  );
  let pending: number | null = null;

  const clearPending = (): void => {
    if (pending !== null) {
      scheduler.clear(pending);
      pending = null;
    }
  };

  // The single path that changes the term, so the URL stays in step. A commit of the
  // term already in force is not mirrored: the leading letters of a query all settle
  // to no term, and each is a keystroke rather than a new search.
  const commitTerm = (next: string | null): void => {
    if (next === term()) {
      return;
    }
    setTerm(next);
    queryUrl.replace(next);
  };

  const input = (value: string): void => {
    setQuery(value);
    clearPending();
    const settled = settledTerm(value);
    // Un-searching does not wait: drop the term now and let the resource defer.
    if (settled === null) {
      commitTerm(null);
      return;
    }
    if (debounce <= 0) {
      commitTerm(settled);
      return;
    }
    pending = scheduler.set(() => {
      pending = null;
      commitTerm(settled);
    }, debounce);
  };

  const settle = (): void => {
    clearPending();
    commitTerm(settledTerm(query()));
  };

  const clear = (): void => {
    clearPending();
    setQuery("");
    commitTerm(null);
  };

  // Derived from the two signals rather than tracked alongside the timer, which a
  // separate flag could disagree with. A field with no searchable word settles to
  // the term it already has, so it is not pending.
  const isPending = (): boolean => settledTerm(query()) !== term();
  const isAwaitingWord = (): boolean =>
    query().trim() !== "" && settledTerm(query()) === null;

  return {
    query,
    term,
    pending: isPending,
    awaitingWord: isAwaitingWord,
    input,
    settle,
    clear,
    cancel: clearPending,
  };
}
