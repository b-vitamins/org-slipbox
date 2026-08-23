/*
 * The glossary dictionary: an `aria-activedescendant` term list beside a peek of
 * the highlighted term, in either of two modes, browse (every term, or a search's
 * matches) or study (the terms due today). Mode arrives as a prop and a change is
 * reported up to the frame that mirrors it to `?view=`; the search goes to `?q=`
 * and the open term to `?term=`, so a definition on screen has an address.
 *
 * One field serves both listings and is named for the one it is over: in browse
 * mode a query is a search of the glossary index, in study mode it narrows the due
 * terms the surface already holds.
 */

import {
  ErrorBoundary,
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  on,
  onCleanup,
  type Component,
} from "solid-js";

import { ApiError, client } from "../api/client.js";
import type { GlossaryTermsResult, NodeRecord } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { WINDOW_SCHEDULER } from "../data/scheduler.js";
import { useDocumentTitle } from "../dom/document-title.js";
import { revealOption } from "../dom/scroll-into-view.js";
import { GlossaryPeek } from "./GlossaryPeek.jsx";
import { browserQueryUrl, type QueryUrl } from "./query-url.js";
import { MIN_TERM_CHARACTERS, createSearchController } from "./search.js";
import { markedIndex, moveSelection } from "./selection.js";
import { dueStanding } from "./study-facts.js";
import { browserTermUrl, type TermUrl } from "./term-url.js";
import "./glossary.css";

/** How many terms one page of a listing or search holds. */
const TERM_LIMIT = 200;
/** Stable id root for listbox options, so `aria-activedescendant` can target them. */
const OPTION_ID = "glossary-option";
/** How near the end of the list, in px, asks for the page after it. */
const CONTINUE_WITHIN = 48;

/** The two list modes: browse the whole glossary, or study what's due. */
export type GlossaryMode = "browse" | "study";

/**
 * The rows of `listed` a filter leaves standing, matched as one case-folded
 * substring of a headword or one of its synonyms. That is the text a listing
 * carries: a definition's body is the index's to search, and the index answers no
 * search behind a due filter, so a term it returned could well not be due.
 */
function narrowToFilter(
  listed: NodeRecord[],
  filter: string | null,
): NodeRecord[] {
  if (filter === null) {
    return listed;
  }
  const needle = filter.toLocaleLowerCase();
  return listed.filter((row) =>
    [row.title, ...row.aliases].some((text) =>
      text.toLocaleLowerCase().includes(needle),
    ),
  );
}

/**
 * `rows` with the first appearance of each key kept. A page read after the first
 * page was re-read can repeat a term the surface already holds, and two rows
 * resolving to one peek is a list a reader cannot walk.
 */
function firstOfEachKey(rows: NodeRecord[]): NodeRecord[] {
  const seen = new Set<string>();
  return rows.filter((row) => {
    if (seen.has(row.node_key)) {
      return false;
    }
    seen.add(row.node_key);
    return true;
  });
}

/** What a first page answered with, and the listing it answered for. */
interface KeptPage {
  readonly listing: string;
  readonly rows: NodeRecord[];
  /** The position it handed out, which the pages after it were read from. */
  readonly boundary: string | null;
}

function describeError(error: unknown): string {
  if (error instanceof ApiError) {
    return `${error.kind}: ${error.message}`;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "the glossary is unreachable";
}

/**
 * The definition pane's standing explanation while the list beside it is empty.
 * Marking and grading both write to a note and this surface is read-only, so it
 * names the tools that write rather than offering a control.
 */
const EmptyGlossary: Component<{
  mode: GlossaryMode;
  searched: boolean;
  /** Whether terms are due and the filter in the box is what emptied the list. */
  filtered: boolean;
}> = (props) => (
  <section class="glossary-empty">
    <Show
      when={props.mode === "study"}
      fallback={
        <>
          <h2 class="glossary-empty__title">How terms enter the glossary</h2>
          <p class="glossary-empty__prose">
            A term is an ordinary note carrying a <code>#+glossary: t</code>{" "}
            marker. Its <code>#+title</code> is the headword, its{" "}
            <code>ROAM_ALIASES</code> are synonyms, and its body is the
            definition, so a term stays searchable, linkable, and backlinked like
            every other note.
          </p>
          <p class="glossary-empty__prose">
            Marking one is an edit, so it happens where notes are written:{" "}
            <code>slipbox glossary mark</code> on the command line, or{" "}
            <code>org-slipbox-glossary-define</code> in Emacs. A marked note
            appears here as soon as the index has seen it.
          </p>
          <Show when={props.searched}>
            <p class="glossary-empty__prose">
              A note answering that search may well be in the slipbox without
              being a term: the Notes surface searches every note, this one only
              the marked ones.
            </p>
          </Show>
        </>
      }
    >
      {/* Terms being due and none matching are different states, so the standing
          explanation of how terms come due gives way to the filter's own account
          rather than answering a question the reader did not ask. */}
      <Show
        when={props.filtered}
        fallback={
          <>
            <h2 class="glossary-empty__title">How terms come due</h2>
            <p class="glossary-empty__prose">
              Each term carries its own review schedule in its property drawer — a
              due date, an interval, and an SM-2 ease that grading moves. Terms
              collect here as their due dates come round.
            </p>
            <p class="glossary-empty__prose">
              Grading rewrites that drawer, so it too happens where notes are
              written: <code>slipbox glossary grade</code>, or the review session in
              Emacs. This surface reads the schedule and shows it beside the
              definition.
            </p>
          </>
        }
      >
        <h2 class="glossary-empty__title">Nothing due matches that filter</h2>
        <p class="glossary-empty__prose">
          Terms are due; the words in the box are over their headwords and synonyms
          and match none of them. Clearing the box brings the rest of what is due
          back.
        </p>
        <p class="glossary-empty__prose">
          Reaching a term that is not due is the other listing's job: All terms
          searches every marked note, definitions included.
        </p>
      </Show>
    </Show>
  </section>
);

export const GlossaryDictionary: Component<{
  /**
   * Open a note as a fresh reading root: a term by its slipbox key, or a note a
   * definition links to by the reference that link carried.
   */
  onOpen: (reference: string) => void;
  /** Which list to show. Owned by the frame, which keeps it in the URL. */
  mode: GlossaryMode;
  /** Report a mode the reader chose, for the owner to record and hand back. */
  onMode: (next: GlossaryMode) => void;
  /** Debounce window (ms) before a keystroke becomes a live search term. */
  debounceMs?: number;
  /** URL seam for the `?q=` term; defaults to the real address bar. */
  queryUrl?: QueryUrl;
  /** URL seam for the `?term=` open term; defaults to the real address bar. */
  termUrl?: TermUrl;
}> = (props) => {
  const queryUrl = props.queryUrl ?? browserQueryUrl();
  const search = createSearchController(
    props.debounceMs ?? 180,
    WINDOW_SCHEDULER,
    queryUrl,
  );
  onCleanup(() => search.cancel());

  const termUrl = props.termUrl ?? browserTermUrl();

  const mode = (): GlossaryMode => props.mode;
  // The peek selection marks a term by key, not by row number; `active` resolves it
  // against the terms on screen (see `markedIndex`). A restored address is a
  // selection like any other, so it seeds the same signal.
  const [marked, setMarked] = createSignal<string | null>(termUrl.read());
  /**
   * The key the address arrived with, until a list holds it or the reader picks
   * another row. A marked term no list holds is ordinarily a term a search or a
   * re-read moved out of view, which the first row stands in for; one the reader
   * asked for by name is not, and has to be answered for rather than replaced.
   */
  const [fromAddress, setFromAddress] = createSignal<string | null>(
    termUrl.read(),
  );

  // Keyed on the mode rather than set from the control's own handler, because the
  // mode also changes from a reload or the back button. The address is where the
  // open term lives, so the list that arrives is read against it again: the term
  // is not cleared here, since a mode change pushes a history entry and the mode
  // is reported before the push, which would strip the term from the entry being
  // left instead of the one being pushed. The term goes off the URL a surface with
  // no listing is pushed at, which is that surface's own encoding.
  createEffect(
    on(
      mode,
      () => {
        const named = termUrl.read();
        setMarked(named);
        setFromAddress(named);
      },
      { defer: true },
    ),
  );

  // In browse mode an absent term is spelled `""`, which still fetches: a reading
  // resource defers only on false, null, and undefined.
  const browsed = createReadingResource(
    () => (mode() === "browse" ? (search.term() ?? "") : false),
    (term) =>
      term === ""
        ? client.glossaryTerms({ limit: TERM_LIMIT })
        : client.searchGlossary(term, { limit: TERM_LIMIT }),
  );
  const due = createReadingResource(
    () => mode() === "study",
    () => client.glossaryDue({ limit: TERM_LIMIT }),
  );

  /** The first page of whichever listing the mode names. */
  const page = (): GlossaryTermsResult | undefined =>
    mode() === "study" ? due.ready() : browsed.ready();

  // The pages read past the first. A reading resource holds one value per key, so
  // a page read through it replaces the page before it instead of extending it;
  // the pages the reader has walked to accumulate here.
  const [appended, setAppended] = createSignal<NodeRecord[]>([]);
  /**
   * Where the next page continues from: null once the listing is spent, and
   * undefined while none has been continued yet, when the first page's own
   * position is the one to echo.
   */
  const [position, setPosition] = createSignal<string | null | undefined>(
    undefined,
  );
  /**
   * The listing a page is in flight for, or null when none is. Keyed, because one
   * listing's flight says nothing about the listing that replaced it: read
   * globally it would report the new list as mid-continuation and refuse to
   * continue it.
   */
  const [continuingListing, setContinuingListing] = createSignal<string | null>(
    null,
  );
  const [pageFailure, setPageFailure] = createSignal<unknown>(undefined);
  /**
   * The position a continuation failed at. The scrollport asks for the page after
   * the rows on every gesture at its end, so a page that failed would go out
   * again on each one; the offer below the list is the way to ask again.
   */
  const [failedPosition, setFailedPosition] = createSignal<string | null>(null);

  /**
   * Which listing is on screen. A change of it starts the pages over, since a
   * position is minted per listing and rows read from one belong to no other. The
   * due filter is not part of it: the field narrows rows already held, and a
   * keystroke that threw those rows away would re-read the listing to narrow it.
   */
  const listingKey = createMemo(() =>
    mode() === "study" ? "due" : `browse:${search.term() ?? ""}`,
  );

  /** Forget every page read past the first, whose order no longer holds. */
  const startPagesOver = (): void => {
    setAppended([]);
    setPosition(undefined);
    setPageFailure(undefined);
    setFailedPosition(null);
  };

  createEffect(on(listingKey, startPagesOver, { defer: true }));

  /**
   * What the first page last answered with. A listing on screen is not owed to the
   * request that refreshed it, so these rows stand in for a re-read that failed;
   * tagged with the listing, so rows of one never stand in another.
   */
  const [keptPage, setKeptPage] = createSignal<KeptPage | null>(null);

  // A first page answering with another boundary is a listing that moved under the
  // pages read past the old one: those rows belong to an order it no longer has,
  // and holding them would show a list no request ever answered with. `on` runs
  // its callback untracked, so the record it compares against is also its own.
  createEffect(
    on(page, (answered) => {
      if (answered === undefined) {
        return;
      }
      const listing = listingKey();
      const boundary = answered.next_position ?? null;
      const kept = keptPage();
      if (kept !== null && kept.listing === listing && kept.boundary !== boundary) {
        startPagesOver();
      }
      setKeptPage({ listing, rows: answered.terms, boundary });
    }),
  );

  /** The first page's rows: the live page, or the ones kept over a failed re-read. */
  const firstRows = (): NodeRecord[] => {
    const answered = page();
    if (answered !== undefined) {
      return answered.terms;
    }
    const kept = keptPage();
    return kept !== null && kept.listing === listingKey() ? kept.rows : [];
  };

  /** Every row read for the listing on screen, in the order its pages arrived. */
  const held = createMemo<NodeRecord[]>(() =>
    firstOfEachKey([...firstRows(), ...appended()]),
  );

  // The due listing takes no query, so the field narrows the rows it answered with
  // rather than asking for a narrower listing.
  const dueShown = createMemo(() => narrowToFilter(held(), search.term()));

  const terms = createMemo<NodeRecord[]>(() =>
    mode() === "study" ? dueShown() : held(),
  );
  const loading = (): boolean =>
    mode() === "study" ? due.loading() : browsed.loading();
  const failure = (): unknown =>
    mode() === "study" ? due.error() : browsed.error();

  /** Whether terms are due and the filter is what left none of them showing. */
  const filterHidDue = (): boolean =>
    mode() === "study" && dueShown().length === 0 && held().length > 0;

  // A page arriving with the addressed term in it settles the question, and the
  // row takes over from here.
  createEffect(() => {
    const key = fromAddress();
    if (key !== null && held().some((row) => row.node_key === key)) {
      setFromAddress(null);
    }
  });

  /**
   * The addressed key while the pages read hold no row for it, which is what asks
   * the index about it. Gated on the listing having answered: before that every
   * key is unheld, and a term about to arrive in the first page needs no lookup.
   */
  const addressUnheld = (): string | false => {
    const key = fromAddress();
    if (key === null || page() === undefined) {
      return false;
    }
    return held().some((row) => row.node_key === key) ? false : key;
  };

  // Either the term exists and is peeked from off the list, or the route refuses
  // the key and the surface says so; both beat standing another term in its place.
  const addressed = createReadingResource(addressUnheld, (key) =>
    client.glossaryTerm(key),
  );

  /**
   * Whether this listing pages at all. Browse and due walk a stored order, so a
   * position in it means the same thing on the next request; a search is ranked by
   * relevance, which no stored key can pick up from, so the index mints no
   * position for one and the whole answer is the one page.
   */
  const continuable = (): boolean =>
    mode() === "study" || search.term() === null;

  /** The token asking for the page after the ones held, or null when none does. */
  const nextPosition = (): string | null => {
    const advanced = position();
    if (advanced !== undefined) {
      return advanced;
    }
    const first = page();
    return first?.has_more ? (first.next_position ?? null) : null;
  };

  /** Whether the listing holds terms the surface has not read. */
  const moreFollow = (): boolean =>
    continuable() ? nextPosition() !== null : (page()?.has_more ?? false);

  /** Whether there is a page to ask for, which only a paged listing has. */
  const canContinue = (): boolean => continuable() && moreFollow();

  /** Whether the listing on screen has a page of its own in flight. */
  const continuing = (): boolean => continuingListing() === listingKey();

  /**
   * How much of the listing the surface is holding, said only while it holds less
   * than all of it. A search says as much and names the remedy it has instead of a
   * continuation, rather than leaving a reader to work out which lists go on.
   */
  const cutStatement = (): string | null => {
    const total = page()?.total;
    if (typeof total !== "number" || !moreFollow()) {
      return null;
    }
    return continuable()
      ? `${held().length} of ${total} terms read.`
      : `${held().length} of ${total} matches shown; a narrower search reaches the rest.`;
  };

  /**
   * Read the page after the ones held and keep it beside them. The position is the
   * one the listing handed out, echoed rather than composed: it is the index's to
   * mint and its to refuse.
   */
  const continueListing = async (): Promise<void> => {
    const after = nextPosition();
    if (after === null || continuing() || !continuable()) {
      return;
    }
    const listing = listingKey();
    setContinuingListing(listing);
    setPageFailure(undefined);
    try {
      const next =
        mode() === "study"
          ? await client.glossaryDue({ limit: TERM_LIMIT, after })
          : await client.glossaryTerms({ limit: TERM_LIMIT, after });
      // A listing the reader left while the page was in flight keeps its own
      // pages: these rows are of a list no longer on screen.
      if (listingKey() === listing) {
        setAppended((rows) => [...rows, ...next.terms]);
        setPosition(next.has_more ? (next.next_position ?? null) : null);
        setFailedPosition(null);
      }
    } catch (error) {
      if (listingKey() === listing) {
        setPageFailure(error);
        setFailedPosition(after);
      }
    } finally {
      setContinuingListing((current) => (current === listing ? null : current));
    }
  };

  /**
   * Continue at the end of the scrollport, unless the page there is the one that
   * failed: a gesture is not a fresh instruction, and a failing page asked for on
   * every one of them is a request storm the reader never made.
   */
  const continueOnScroll = (): void => {
    if (nextPosition() !== failedPosition()) {
      void continueListing();
    }
  };

  // `search.term()` is null exactly when the field holds no searchable word, which
  // is the "no search yet" case.
  const emptyMessage = (): string => {
    if (mode() === "study") {
      return filterHidDue()
        ? "No terms due for review match that filter."
        : "Nothing is due for review.";
    }
    return search.term() === null
      ? "The glossary has no terms yet."
      : "No terms match that search.";
  };

  /**
   * The size and the order of the due listing, in one line. The count is the
   * listing's own total rather than the rows on the surface, which is a smaller
   * number the reader would read as the answer. The order is named rather than
   * re-sorted here: the surface holds one page of a listing it did not sort, and a
   * corpus no review has touched carries no dates for an order to be read off.
   */
  const dueSummary = (): string | null => {
    const listing = due.ready();
    if (mode() !== "study" || !listing || !(listing.total > 0)) {
      return null;
    }
    const counted =
      listing.total === 1 ? "1 term is" : `${listing.total} terms are`;
    return `${counted} due, in schedule order: never reviewed first, then by due date, then by file path.`;
  };

  /** What the field is over, which is what it does: the box says both. */
  const fieldLabel = (): string =>
    mode() === "study"
      ? "Filter the terms due for review"
      : "Search the glossary";

  // Named per mode, so two open modes stay distinguishable in the tab strip.
  useDocumentTitle(() =>
    mode() === "study" ? "Glossary review" : "Glossary",
  );

  const listboxId = "glossary-terms";
  const optionId = (index: number): string => `${OPTION_ID}-${index}`;

  // A list that no longer holds the marked term falls back to its first, so the peek
  // is never blank beside a non-empty list. Not for a term the address named: that
  // one is reported on instead, since a definition under someone else's address is
  // read as the answer to it.
  const active = createMemo<number | null>(() => {
    const resolved = markedIndex(marked(), terms(), (term) => term.node_key);
    if (resolved !== null) {
      return resolved;
    }
    if (fromAddress() !== null) {
      return null;
    }
    return terms().length > 0 ? 0 : null;
  });

  /** Move the peek to a row of the current list, or off the list entirely. */
  const markRow = (index: number | null): void => {
    const key = index === null ? null : (terms()[index]?.node_key ?? null);
    setMarked(key);
    setFromAddress(null);
    termUrl.replace(key);
  };

  // The fallback above stands the first row in for a marked term the list dropped,
  // which leaves the address naming one definition beside another on screen. The
  // address follows the peek it moved: it is a replacement like every other
  // selection, since no page was walked to. An address naming nothing claims
  // nothing, so a listing read with no term named is left alone.
  createEffect(() => {
    const named = marked();
    const index = active();
    if (named === null || index === null) {
      return;
    }
    const shown = terms()[index]?.node_key ?? null;
    if (shown !== null && shown !== named) {
      setMarked(shown);
      termUrl.replace(shown);
    }
  });

  /** The addressed term read from the index, while no row on screen is it. */
  const offList = (): NodeRecord | undefined =>
    active() === null ? (addressed.ready()?.term ?? undefined) : undefined;

  const selected = (): NodeRecord | undefined => {
    const index = active();
    return index === null ? offList() : terms()[index];
  };

  /** Why a peeked term is nowhere in the list beside it. */
  const offListStatement = (): string | null => {
    const off = offList();
    return off === undefined
      ? null
      : `${off.title} is not among the terms read so far.`;
  };

  /**
   * The addressed key the index refuses. A key is one of a note's names rather
   * than a term's headword, so the reader is owed the name that failed: it is what
   * a stale bookmark, a renamed file, and an unmarked note look like from here.
   */
  const unknownAddress = (): string | null => {
    const key = addressUnheld();
    return key !== false && addressed.error() !== undefined
      ? `No glossary term is named ${key}.`
      : null;
  };
  const activeId = (): string | undefined => {
    const index = active();
    return index === null ? undefined : optionId(index);
  };

  // The cursor is an `aria-activedescendant`, not focus, so the browser does not
  // scroll it into view; this list scrolls its own box.
  createEffect(() => {
    revealOption(activeId());
  });

  // Two filters over one list, not two panels: each is a toggle whose pressed state
  // says which filter holds, and both are ordinary tab stops. That leaves every
  // arrow key to the term list below, which is the widget the arrows drive.
  const FILTERS: readonly { id: GlossaryMode; label: string }[] = [
    { id: "browse", label: "All terms" },
    { id: "study", label: "Due for review" },
  ];

  /**
   * Report a filter the reader pressed, unless it is the one already holding. The
   * owner mirrors a mode to a pushed history entry, so re-reporting the mode on
   * screen stacks entries that undo nothing: a reader pressing "All terms" twice
   * would have to press back twice to leave the list they never left.
   */
  const show = (next: GlossaryMode): void => {
    if (next !== mode()) {
      props.onMode(next);
    }
  };

  const onKeyDown = (event: KeyboardEvent): void => {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        markRow(moveSelection(active(), "next", terms().length));
        break;
      case "ArrowUp":
        event.preventDefault();
        markRow(moveSelection(active(), "previous", terms().length));
        break;
      case "Enter": {
        event.preventDefault();
        // The listed terms answer the settled term, not the field, so settle first:
        // inside the debounce window they match a query already replaced.
        if (search.pending()) {
          search.settle();
          break;
        }
        const term = selected();
        if (term) {
          props.onOpen(term.node_key);
        }
        break;
      }
    }
  };

  return (
    <main class="glossary">
      <div class="glossary-list">
        {/* The top of the outline, above every conditional block below it: the
            peek's headword is an `h2` and a definition's headings nest under
            that, so the surface reads as one document in every state. */}
        <h1 class="glossary-title">Glossary</h1>
        <div class="glossary-modes" role="group" aria-label="Glossary listing">
          <For each={FILTERS}>
            {(filter) => (
              <button
                type="button"
                aria-pressed={mode() === filter.id}
                // Named only while there is a list: an idref resolving to nothing
                // would announce a relationship the surface is not holding.
                aria-controls={terms().length > 0 ? listboxId : undefined}
                class="glossary-mode"
                onClick={() => show(filter.id)}
              >
                {filter.label}
              </button>
            )}
          </For>
        </div>

        {/* One element across both modes, so a mode change neither takes the field
            away nor drops the focus and text it was holding. */}
        <input
          type="search"
          class="glossary-search"
          placeholder={fieldLabel()}
          autocomplete="off"
          aria-label={fieldLabel()}
          role="combobox"
          aria-expanded={terms().length > 0}
          // Named only while the popup is rendered, as the mode controls above
          // are: an idref resolving to nothing announces a relationship the
          // surface is not holding.
          aria-controls={terms().length > 0 ? listboxId : undefined}
          aria-activedescendant={activeId()}
          value={search.query()}
          onInput={(event) => search.input(event.currentTarget.value)}
          onKeyDown={onKeyDown}
        />

        <Show when={search.awaitingWord()}>
          <p class="glossary-status glossary-status--hint">
            {mode() === "study" ? "Filtering" : "Searching"} needs a word of at
            least {MIN_TERM_CHARACTERS} characters.
          </p>
        </Show>

        <Show when={dueSummary()}>
          {(summary) => (
            <p class="glossary-status glossary-status--hint">{summary()}</p>
          )}
        </Show>

        {/* Above the rows rather than in place of them: a re-read that failed
            takes nothing away, and a listing the reader is holding is not owed
            to the request that refreshed it. */}
        <Show when={failure()}>
          <p class="glossary-status glossary-status--error">
            {describeError(failure())}
          </p>
        </Show>

        <Show
          when={terms().length > 0}
          fallback={
            <Show when={!failure()}>
              <Show
                when={!loading()}
                fallback={<p class="glossary-status">Reading the glossary…</p>}
              >
                <p class="glossary-status">{emptyMessage()}</p>
              </Show>
            </Show>
          }
        >
          {/* The field is the tab stop in both modes and carries the cursor, so
              the popup it controls takes neither and needs no key handler of its
              own. */}
          <ul
            id={listboxId}
            role="listbox"
            aria-label="Glossary terms"
            class="glossary-terms"
            // The list scrolls its own box, so its end is where the reader
            // reaches the end of what has been read, and what asks for more.
            onScroll={(event) => {
              const box = event.currentTarget;
              const past = box.scrollHeight - box.scrollTop - box.clientHeight;
              if (past <= CONTINUE_WITHIN) {
                continueOnScroll();
              }
            }}
          >
            <For each={terms()}>
              {(term, index) => (
                <li
                  id={optionId(index())}
                  role="option"
                  aria-selected={active() === index()}
                  class="glossary-term"
                  classList={{
                    "glossary-term--active": active() === index(),
                  }}
                  // A pointer only selects the row to peek it; opening is the
                  // peek's own control or Enter. Click as well as hover, since a
                  // touch pointer has no hover to select with.
                  onMouseEnter={() => markRow(index())}
                  onClick={() => markRow(index())}
                >
                  <span class="glossary-term__title">{term.title}</span>
                  {/* Only where the reader is reading the schedule: in browse
                      mode a standing would be a fact about a listing that is not
                      the one on screen. */}
                  <Show when={mode() === "study" && dueStanding(term)}>
                    {(standing) => (
                      <span class="glossary-term__standing">{standing()}</span>
                    )}
                  </Show>
                  <Show when={term.glossary_status === "stub"}>
                    <span class="glossary-term__stub">stub</span>
                  </Show>
                </li>
              )}
            </For>
          </ul>
        </Show>

        <Show when={unknownAddress()}>
          {(statement) => (
            <p class="glossary-status glossary-status--error">{statement()}</p>
          )}
        </Show>
        <Show when={offListStatement()}>
          {(statement) => (
            <p class="glossary-status glossary-status--hint">{statement()}</p>
          )}
        </Show>

        {/* Below the scrollport rather than inside it: a control the list scrolls
            away is one a reader has to find, and a listing whose rows a filter
            hid still has a cut to state. */}
        <Show when={cutStatement()}>
          {(statement) => (
            <p class="glossary-status glossary-status--hint">{statement()}</p>
          )}
        </Show>
        <Show when={pageFailure()}>
          <p class="glossary-status glossary-status--error">
            More terms could not be read: {describeError(pageFailure())}
          </p>
        </Show>
        <Show when={canContinue()}>
          <button
            type="button"
            class="glossary-more"
            disabled={continuing()}
            onClick={() => void continueListing()}
          >
            {continuing() ? "Reading more terms…" : "Read more terms"}
          </button>
        </Show>
      </div>

      <div class="glossary-detail">
        <Show
          when={selected()}
          fallback={
            // Neither message is shown while the list is still being read, since a
            // read that resolves into terms would flash an explanation of their
            // absence first. A term the address named is being read for the same
            // reason: it may yet fill this pane.
            <Show when={!loading() && !failure() && !addressed.loading()}>
              <Show
                when={terms().length > 0}
                fallback={
                  <EmptyGlossary
                    mode={mode()}
                    searched={search.term() !== null}
                    filtered={filterHidDue()}
                  />
                }
              >
                <p class="glossary-status">
                  Select a term to read its definition.
                </p>
              </Show>
            </Show>
          }
        >
          {(term) => (
            // A definition the renderer cannot draw throws out of the update that
            // resolved it, where no branch in the peek is watching.
            //
            // Keyed to the term because a caught error latches until the boundary
            // that caught it is discarded: one boundary for the pane would hold the
            // message over every term walked to afterwards. Keyed on the key rather
            // than the record, so re-reading the list leaves the peek's fetch alone.
            //
            // The fallback declares the error parameter even though it goes unread:
            // a boundary whose fallback takes no error has not handled it, and Solid
            // re-reports it to the console.
            <Show when={term().node_key} keyed>
              <ErrorBoundary
                fallback={(_error) => (
                  <p class="glossary-status glossary-status--error">
                    This definition could not be rendered.
                  </p>
                )}
              >
                <GlossaryPeek term={term()} onOpen={props.onOpen} />
              </ErrorBoundary>
            </Show>
          )}
        </Show>
      </div>
    </main>
  );
};
