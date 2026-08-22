/*
 * The glossary dictionary: an `aria-activedescendant` term list beside a peek of
 * the highlighted term, in either of two modes, browse (every term, or a search's
 * matches) or study (the terms due today). Mode arrives as a prop and a change is
 * reported up to the frame that mirrors it to `?view=`; the term goes to `?q=`.
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
import "./glossary.css";

/** How many terms a listing or search shows. */
const TERM_LIMIT = 200;
/** Stable id root for listbox options, so `aria-activedescendant` can target them. */
const OPTION_ID = "glossary-option";

/** The two list modes: browse the whole glossary, or study what's due. */
export type GlossaryMode = "browse" | "study";

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
const EmptyGlossary: Component<{ mode: GlossaryMode; searched: boolean }> = (
  props,
) => (
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
      <h2 class="glossary-empty__title">How terms come due</h2>
      <p class="glossary-empty__prose">
        Each term carries its own review schedule in its property drawer — a due
        date, an interval, and an SM-2 ease that grading moves. Terms collect here
        as their due dates come round.
      </p>
      <p class="glossary-empty__prose">
        Grading rewrites that drawer, so it too happens where notes are written:{" "}
        <code>slipbox glossary grade</code>, or the review session in Emacs. This
        surface reads the schedule and shows it beside the definition.
      </p>
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

  // Keyed on the mode rather than set from the tab handler, because the mode also
  // changes from a reload or the back button.
  //
  // The roving tabindex below is derived from the mode, so this arrival moves the
  // tablist's single tab stop: focus left on the tab it moved off would sit outside
  // the tab order, and the next Tab would leave the tablist entirely. Focus follows
  // the stop only while the tablist holds it, since taking focus from the search
  // field or the term list would take the keyboard with it.
  createEffect(
    on(
      mode,
      (next) => {
        setMarked(null);
        const held = document.activeElement;
        if (tabRefs.some((tab) => tab === held)) {
          tabRefs[TABS.findIndex((tab) => tab.id === next)]?.focus();
        }
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

  const terms = createMemo<NodeRecord[]>(() => {
    const result = mode() === "study" ? due.ready() : browsed.ready();
    return result?.terms ?? [];
  });
  const loading = (): boolean =>
    mode() === "study" ? due.loading() : browsed.loading();
  const failure = (): unknown =>
    mode() === "study" ? due.error() : browsed.error();

  // `search.term()` is null exactly when the field holds no searchable word, which
  // is the "no search yet" case.
  const emptyMessage = (): string => {
    if (mode() === "study") {
      return "Nothing is due for review.";
    }
    return search.term() === null
      ? "The glossary has no terms yet."
      : "No terms match that search.";
  };

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

  // Study mode has no search box, so the listbox itself is the focusable widget
  // carrying the arrow-key cursor; in browse mode the combobox owns focus.
  const listboxOwnsFocus = (): boolean => mode() === "study";

  // A WAI-ARIA tablist: one tab stop (the roving tabindex below), the horizontal
  // arrows walk between tabs and activate on focus, Home and End jump to the ends.
  // Only the horizontal arrows, so the vertical pair stays with the term list.
  const TABS: readonly { id: GlossaryMode; label: string }[] = [
    { id: "browse", label: "All terms" },
    { id: "study", label: "Due for review" },
  ];
  const tabRefs: HTMLButtonElement[] = [];

  const onTabKeyDown = (event: KeyboardEvent, index: number): void => {
    const last = TABS.length - 1;
    let next: number;
    switch (event.key) {
      case "ArrowRight":
        next = index === last ? 0 : index + 1;
        break;
      case "ArrowLeft":
        next = index === 0 ? last : index - 1;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = last;
        break;
      default:
        return;
    }
    event.preventDefault();
    props.onMode(TABS[next]!.id);
    tabRefs[next]?.focus();
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
        <div
          class="glossary-modes"
          role="tablist"
          aria-orientation="horizontal"
          aria-label="Glossary mode"
        >
          <For each={TABS}>
            {(tab, index) => (
              <button
                ref={(element) => (tabRefs[index()] = element)}
                type="button"
                role="tab"
                aria-selected={mode() === tab.id}
                // Roving tabindex: only the selected tab is in the tab order.
                tabindex={mode() === tab.id ? undefined : -1}
                class="glossary-mode"
                classList={{ "glossary-mode--active": mode() === tab.id }}
                onClick={() => props.onMode(tab.id)}
                onKeyDown={(event) => onTabKeyDown(event, index())}
              >
                {tab.label}
              </button>
            )}
          </For>
        </div>

        <Show when={mode() === "browse"}>
          <input
            type="search"
            class="glossary-search"
            placeholder="Search the glossary"
            autocomplete="off"
            aria-label="Search the glossary"
            role="combobox"
            aria-expanded={terms().length > 0}
            aria-controls={listboxId}
            aria-activedescendant={activeId()}
            value={search.query()}
            onInput={(event) => search.input(event.currentTarget.value)}
            onKeyDown={onKeyDown}
          />
        </Show>

        <Show when={search.awaitingWord()}>
          <p class="glossary-status glossary-status--hint">
            Searching needs a word of at least {MIN_TERM_CHARACTERS} characters.
          </p>
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
            <ul
              id={listboxId}
              role="listbox"
              aria-label="Glossary terms"
              class="glossary-terms"
              tabindex={listboxOwnsFocus() ? 0 : undefined}
              aria-activedescendant={
                listboxOwnsFocus() ? activeId() : undefined
              }
              onKeyDown={onKeyDown}
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
