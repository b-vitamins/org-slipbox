/*
 * The glossary dictionary: an `aria-activedescendant` term list beside a peek of
 * the highlighted term, in either of two modes, browse (every term, or a search's
 * matches) or study (the terms due today). Mode arrives as a prop and a change is
 * reported up to the frame that mirrors it to `?view=`; the term goes to `?q=`.
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
import type { NodeRecord } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { WINDOW_SCHEDULER } from "../data/scheduler.js";
import { useDocumentTitle } from "../dom/document-title.js";
import { revealOption } from "../dom/scroll-into-view.js";
import { GlossaryPeek } from "./GlossaryPeek.jsx";
import { browserQueryUrl, type QueryUrl } from "./query-url.js";
import { MIN_TERM_CHARACTERS, createSearchController } from "./search.js";
import { markedIndex, moveSelection } from "./selection.js";
import { dueStanding } from "./study-facts.js";
import "./glossary.css";

/** How many terms a listing or search shows. */
const TERM_LIMIT = 200;
/** Stable id root for listbox options, so `aria-activedescendant` can target them. */
const OPTION_ID = "glossary-option";

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
}> = (props) => {
  const queryUrl = props.queryUrl ?? browserQueryUrl();
  const search = createSearchController(
    props.debounceMs ?? 180,
    WINDOW_SCHEDULER,
    queryUrl,
  );
  onCleanup(() => search.cancel());

  const mode = (): GlossaryMode => props.mode;
  // The peek selection marks a term by key, not by row number; `active` resolves it
  // against the terms on screen (see `markedIndex`).
  const [marked, setMarked] = createSignal<string | null>(null);

  // Keyed on the mode rather than set from the control's own handler, because the
  // mode also changes from a reload or the back button. The two lists share no
  // rows, so the peek starts again from the first row of the list that arrives.
  createEffect(
    on(
      mode,
      () => {
        setMarked(null);
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

  // The due listing takes no query, so the field narrows the rows it answered with
  // rather than asking for a narrower listing.
  const dueListed = (): NodeRecord[] => due.ready()?.terms ?? [];
  const dueShown = createMemo(() => narrowToFilter(dueListed(), search.term()));

  const terms = createMemo<NodeRecord[]>(() =>
    mode() === "study" ? dueShown() : (browsed.ready()?.terms ?? []),
  );
  const loading = (): boolean =>
    mode() === "study" ? due.loading() : browsed.loading();
  const failure = (): unknown =>
    mode() === "study" ? due.error() : browsed.error();

  /** Whether terms are due and the filter is what left none of them showing. */
  const filterHidDue = (): boolean =>
    mode() === "study" && dueShown().length === 0 && dueListed().length > 0;

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
  // is never blank beside a non-empty list.
  const active = createMemo<number | null>(() => {
    const resolved = markedIndex(marked(), terms(), (term) => term.node_key);
    if (resolved !== null) {
      return resolved;
    }
    return terms().length > 0 ? 0 : null;
  });

  /** Move the peek to a row of the current list, or off the list entirely. */
  const markRow = (index: number | null): void => {
    setMarked(index === null ? null : (terms()[index]?.node_key ?? null));
  };

  const selected = (): NodeRecord | undefined => {
    const index = active();
    return index === null ? undefined : terms()[index];
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

        <Show
          when={!failure()}
          fallback={
            <p class="glossary-status glossary-status--error">
              {describeError(failure())}
            </p>
          }
        >
          <Show
            when={terms().length > 0}
            fallback={
              <Show
                when={!loading()}
                fallback={<p class="glossary-status">Reading the glossary…</p>}
              >
                <p class="glossary-status">{emptyMessage()}</p>
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
        </Show>
      </div>

      <div class="glossary-detail">
        <Show
          when={selected()}
          fallback={
            // Neither message is shown while the list is still being read, since a
            // read that resolves into terms would flash an explanation of their
            // absence first.
            <Show when={!loading() && !failure()}>
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
