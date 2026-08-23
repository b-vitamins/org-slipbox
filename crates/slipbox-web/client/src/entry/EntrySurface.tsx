/*
 * The search-first entry surface: an identity hero, a debounced content search over
 * a listbox of notes, and a random open. The field is a WAI-ARIA combobox, so the
 * arrow keys move an `aria-activedescendant` highlight without stealing the caret.
 * The settled term is mirrored to `?q=`, the cursor to the history entry.
 */

import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type Component,
} from "solid-js";

import { ApiError, client } from "../api/client.js";
import type { NodeContentHit, StatusInfo } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { WINDOW_SCHEDULER } from "../data/scheduler.js";
import { useDocumentTitle } from "../dom/document-title.js";
import { revealOption } from "../dom/scroll-into-view.js";
import { RenderInline } from "../org/RenderInline.jsx";
import { browserCursorHistory, type CursorHistory } from "./cursor-history.js";
import { excerptRuns } from "./excerpt.js";
import { slipboxName } from "./identity.js";
import { browserQueryUrl, type QueryUrl } from "./query-url.js";
import { MIN_TERM_CHARACTERS, createSearchController } from "./search.js";
import { markedIndex, moveSelection } from "./selection.js";
import "./entry.css";

/** How many results a single search shows. */
const SEARCH_LIMIT = 20;
/** Stable id root for listbox options, so `aria-activedescendant` can target them. */
const OPTION_ID = "entry-option";

function describeError(error: unknown): string {
  if (error instanceof ApiError) {
    return `${error.kind}: ${error.message}`;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "the reading surface is unreachable";
}

/** The served slipbox identity, the resting hero above the search field. */
const IdentityHero: Component<{ info: StatusInfo }> = (props) => (
  <section class="entry-hero">
    {/* The name, not the absolute path, which leaks a local username and directory
        into every screenshot; the path stays on hover. */}
    <h1 class="entry-hero__root" title={props.info.root}>
      {slipboxName(props.info.root)}
    </h1>
    <p class="entry-hero__facts">
      {/* `notes_indexed` counts file nodes and id-bearing headings, the ones a
          reader can reach. `nodes_indexed` also counts plain headings, which the
          search and random surfaces never return. */}
      <span>{props.info.notes_indexed} notes</span>
      {" · "}
      <span>{props.info.links_indexed} links</span>
      {" · "}
      <span>slipbox {props.info.version}</span>
    </p>
  </section>
);

/**
 * One matched note: title over its tags and the excerpt it matched in, highlighted
 * when it is the cursor.
 *
 * The excerpt renders run by run as preview prose (`excerpt.ts`), the matched runs
 * marked. The highlight is carried as flagged data, so a literal `<mark>` in a note
 * stays the characters it spells.
 */
const ResultRow: Component<{
  hit: NodeContentHit;
  id: string;
  active: boolean;
  onChoose: () => void;
  onHover: () => void;
}> = (props) => {
  const runs = createMemo(() => excerptRuns(props.hit.snippet.segments));
  return (
    <li
      id={props.id}
      role="option"
      aria-selected={props.active}
      class="entry-result"
      classList={{ "entry-result--active": props.active }}
      // A mouse press must not blur the input before the click opens the note.
      onMouseDown={(event) => event.preventDefault()}
      onMouseEnter={props.onHover}
      onClick={props.onChoose}
    >
      <span class="entry-result__title">{props.hit.node.title}</span>
      <Show when={props.hit.node.tags.length > 0}>
        <span class="entry-result__tags">
          <For each={props.hit.node.tags}>
            {(tag) => <span class="entry-result__tag">{tag}</span>}
          </For>
        </span>
      </Show>
      <Show when={runs().length > 0}>
        <span class="entry-result__snippet">
          <For each={runs()}>
            {(run) => (
              <Show when={run.matched} fallback={<RenderInline nodes={run.prose} />}>
                <mark class="entry-result__match">
                  <RenderInline nodes={run.prose} />
                </mark>
              </Show>
            )}
          </For>
        </span>
      </Show>
    </li>
  );
};

export const EntrySurface: Component<{
  /** Open a note as a fresh reading root, by slipbox key. */
  onOpen: (key: string) => void;
  /** Debounce window (ms) before a keystroke becomes a live search term. */
  debounceMs?: number;
  /** URL seam for the `?q=` term; defaults to the real address bar. */
  queryUrl?: QueryUrl;
  /** History seam for the result cursor; defaults to the real history entry. */
  cursorHistory?: CursorHistory;
}> = (props) => {
  const queryUrl = props.queryUrl ?? browserQueryUrl();
  const cursorHistory = props.cursorHistory ?? browserCursorHistory();
  const search = createSearchController(
    props.debounceMs ?? 180,
    WINDOW_SCHEDULER,
    queryUrl,
  );
  onCleanup(() => search.cancel());

  // The arrow cursor marks a note by key, not by row number; `active` resolves it
  // against the notes on screen (see `markedIndex`). Seeded from the history entry,
  // and a key absent from the results marks nothing.
  const [marked, setMarked] = createSignal<string | null>(cursorHistory.read());
  // The term whose results an Enter is waiting to open. Holding the term rather
  // than a flag is what keeps only that term's results able to spend the open.
  const [awaitedOpen, setAwaitedOpen] = createSignal<string | null>(null);
  const [randomPending, setRandomPending] = createSignal(false);
  const [randomError, setRandomError] = createSignal<unknown>(null);

  const status = createReadingResource(
    () => true,
    () => client.status(),
  );
  // Content search, not metadata search: the content index holds each note's title
  // and aliases alongside its body, so this path is a superset of the metadata one.
  const results = createReadingResource(search.term, (term) =>
    client.searchContent(term, { limit: SEARCH_LIMIT }),
  );

  // Only the error state names the tab; note titles arrive from the spine.
  useDocumentTitle(() => (status.error() ? "Unavailable" : undefined));

  const hits = createMemo<NodeContentHit[]>(() => results.ready()?.hits ?? []);
  const active = createMemo<number | null>(() =>
    markedIndex(marked(), hits(), (hit) => hit.node.node_key),
  );
  /** Move the cursor to a row of the current list, or off the list entirely. */
  const markRow = (index: number | null): void => {
    setMarked(index === null ? null : (hits()[index]?.node.node_key ?? null));
  };
  const hasQuery = (): boolean => search.term() !== null;
  // A full page means the server may have more it did not send.
  const atLimit = (): boolean => hits().length >= SEARCH_LIMIT;
  // `aria-expanded` must track whether the listbox is in the DOM, not whether a
  // term exists.
  const isExpanded = (): boolean => hasQuery() && hits().length > 0;
  const listboxId = "entry-results";
  const optionId = (index: number): string => `${OPTION_ID}-${index}`;

  /**
   * What the surface found, in one line. The count is stated here and nowhere
   * else - the capped list's line below carries the advice alone, so the two do
   * not read as one fact twice.
   *
   * `Searching…` is reserved for a term with nothing in hand: a re-read keeps the
   * notes it already has, and saying it again over those would announce a search
   * the reader can already see the results of.
   */
  const summary = (): string => {
    if (!hasQuery()) {
      return "";
    }
    const count = hits().length;
    if (count === 0) {
      return results.loading() ? "Searching…" : "No notes match that search.";
    }
    if (atLimit()) {
      return `Showing the first ${SEARCH_LIMIT} matches.`;
    }
    return count === 1 ? "1 note matches." : `${count} notes match.`;
  };

  /** The failure the surface is left reporting, or null when there is none. */
  const failure = (): unknown =>
    status.error() ?? randomError() ?? results.error() ?? null;

  /**
   * The live region's whole content. Every state the surface reports in words
   * reads out of this one element, because a region announces nothing about text
   * it already held when it was inserted: a paragraph mounted with a failure in
   * it says that failure to nobody.
   *
   * A memo, so a re-render under the same query rewrites no text node and the
   * region announces nothing. A failure outranks the search state, since it is
   * the answer to what the reader just did.
   */
  const announcement = createMemo((): string => {
    const failed = failure();
    if (failed !== null) {
      return describeError(failed);
    }
    if (search.awaitingWord()) {
      return `Searching needs a word of at least ${MIN_TERM_CHARACTERS} characters.`;
    }
    return summary();
  });

  const activeId = (): string | undefined => {
    const index = active();
    return index === null ? undefined : optionId(index);
  };
  /** The highlighted note's key: the mark, once resolved against the list. */
  const activeKey = (): string | null => {
    const index = active();
    return index === null ? null : (hits()[index]?.node.node_key ?? null);
  };

  // The cursor is an `aria-activedescendant`, not focus, so the browser does not
  // scroll it into view. Keyed on the id, so it runs once the row is rendered.
  createEffect(() => {
    revealOption(activeId());
  });

  // On a Back onto this surface the browser restores focus to the element that had
  // it, which unmounted with the previous surface, so focus would fall to the body
  // and the arrow keys would scroll the page instead of walking the results.
  let field: HTMLInputElement | undefined;
  onMount(() => field?.focus());

  /**
   * Leave the surface for a note, recording the cursor on the entry being left: the
   * note itself when it is one of the listed ones, otherwise the standing highlight,
   * which is what a random open leaves in place.
   *
   * What is recorded is the highlight resolved against the list rather than the raw
   * mark, so the entry only carries a row that was there.
   *
   * The write must precede `onOpen`, which pushes the opened note's own history
   * entry: a write after it would annotate that entry instead.
   */
  const leaveFor = (key: string): void => {
    const listed = hits().some((hit) => hit.node.node_key === key);
    cursorHistory.write(listed ? key : activeKey());
    props.onOpen(key);
  };

  const open = (hit: NodeContentHit | undefined): void => {
    if (hit) {
      leaveFor(hit.node.node_key);
    }
  };

  // Honour the open an Enter left standing, once the results for the term it settled
  // are in hand. The open is spent before the note is reached, so a later answer to
  // the same term (a refocus re-read among them) is not a second press. An empty or
  // failed answer spends it too, since an open still standing would fire into
  // whatever search came next.
  createEffect(() => {
    const awaited = awaitedOpen();
    if (awaited === null || search.term() !== awaited) {
      return;
    }
    if (results.ready() === undefined && results.error() === undefined) {
      // Neither answered nor failed: the request is still out, so the notes on hand
      // still answer an earlier search.
      return;
    }
    setAwaitedOpen(null);
    open(hits()[active() ?? 0]);
  });

  // A keystroke replaces the query, abandoning an open standing for the old one.
  const onQueryInput = (value: string): void => {
    search.input(value);
    setAwaitedOpen(null);
    // A failed random open is stale once a search is under way, and the one line
    // the surface speaks through would otherwise keep reporting it instead of the
    // count.
    setRandomError(null);
  };

  const openRandom = async (): Promise<void> => {
    // Ignore a second press while one is in flight: two random opens would race.
    if (randomPending()) {
      return;
    }
    setRandomPending(true);
    setRandomError(null);
    try {
      const result = await client.randomNode();
      if (result.node) {
        leaveFor(result.node.node_key);
      }
    } catch (error) {
      setRandomError(error);
    } finally {
      setRandomPending(false);
    }
  };

  const onKeyDown = (event: KeyboardEvent): void => {
    switch (event.key) {
      // Arrowing makes the choice an outstanding Enter would make, so it abandons it.
      case "ArrowDown":
        event.preventDefault();
        setAwaitedOpen(null);
        markRow(moveSelection(active(), "next", hits().length));
        break;
      case "ArrowUp":
        event.preventDefault();
        setAwaitedOpen(null);
        markRow(moveSelection(active(), "previous", hits().length));
        break;
      case "Enter": {
        event.preventDefault();
        // Inside the debounce window the listed notes answer an earlier query than
        // the field shows, so Enter settles the query and holds the open for that
        // term's results. A field with no searchable word settles to no term, and
        // nothing will be fetched to answer it, so no open is left standing.
        if (search.pending()) {
          search.settle();
          setAwaitedOpen(search.term());
          break;
        }
        open(hits()[active() ?? 0]);
        break;
      }
      case "Escape":
        event.preventDefault();
        setAwaitedOpen(null);
        search.clear();
        setMarked(null);
        break;
    }
  };

  return (
    <main class="entry">
      {/* The unreachable branch stops at the live region rather than enclosing
          it: a branch that both removes the region and states the failure states
          it to nobody. */}
      <Show when={!status.error()}>
        <Show when={status.ready()}>{(info) => <IdentityHero info={info()} />}</Show>
        <div class="entry-search">
          <input
            ref={field}
            type="search"
            class="entry-search__field"
            placeholder="Search notes"
            autocomplete="off"
            aria-label="Search notes"
            role="combobox"
            aria-expanded={isExpanded()}
            aria-controls={listboxId}
            aria-activedescendant={activeId()}
            value={search.query()}
            onInput={(event) => onQueryInput(event.currentTarget.value)}
            onKeyDown={onKeyDown}
          />
          <button
            type="button"
            class="entry-search__random"
            // The field owns focus; a press here must not blur it first.
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => void openRandom()}
            disabled={randomPending()}
            aria-busy={randomPending()}
          >
            Surprise me
          </button>
        </div>
      </Show>

      {/* The live region, placed before there is anything to say: a region
          announces nothing it already held when it arrived. The list stays
          outside it, since re-reading every row is the announcement this
          replaces. */}
      <p
        class="entry-status entry-status--summary"
        classList={{
          "entry-status--error": failure() !== null,
          "entry-status--hint": failure() === null && search.awaitingWord(),
        }}
        role="status"
      >
        {announcement()}
      </p>

      {/* Notes in hand, rather than a term: a failed or unanswered search holds
          none, and those states are what the line above is left saying. */}
      <Show when={!status.error() && hits().length > 0}>
        <ul
          id={listboxId}
          role="listbox"
          aria-label="Search results"
          class="entry-results"
        >
          <For each={hits()}>
            {(hit, index) => (
              <ResultRow
                hit={hit}
                id={optionId(index())}
                active={active() === index()}
                onChoose={() => open(hit)}
                onHover={() => markRow(index())}
              />
            )}
          </For>
        </ul>
        <Show when={atLimit()}>
          <p class="entry-status entry-status--more">
            Refine your search to narrow it.
          </p>
        </Show>
      </Show>
    </main>
  );
};
