import {
  ErrorBoundary,
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  on,
  onCleanup,
  onMount,
  type Component,
} from "solid-js";

import { ApiError, client } from "../api/client.js";
import type { GlossaryTermsResult, NodeRecord } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { WINDOW_SCHEDULER } from "../data/scheduler.js";
import { useDocumentTitle } from "../dom/document-title.js";
import { createHoverMotion } from "../dom/hover-motion.js";
import { revealOption } from "../dom/scroll-into-view.js";
import { GlossaryPeek } from "./GlossaryPeek.jsx";
import { browserQueryUrl, type QueryUrl } from "./query-url.js";
import { MIN_TERM_CHARACTERS, createSearchController } from "./search.js";
import { markedIndex, moveSelection } from "./selection.js";
import { dueStanding } from "./study-facts.js";
import { browserTermUrl, type TermUrl } from "./term-url.js";
import "./glossary.css";

const TERM_LIMIT = 200;
const OPTION_ID = "glossary-option";
const CONTINUE_WITHIN = 48;

export type GlossaryMode = "browse" | "study";

/** Deduplicate page boundaries while preserving listing order. */
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

interface KeptPage {
  readonly listing: string;
  readonly rows: NodeRecord[];
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

const EmptyGlossary: Component<{
  mode: GlossaryMode;
  searched: boolean;
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
  onOpen: (reference: string) => void;
  mode: GlossaryMode;
  onMode: (next: GlossaryMode) => void;
  debounceMs?: number;
  queryUrl?: QueryUrl;
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
  const [detailOpen, setDetailOpen] = createSignal(termUrl.read() !== null);
  const [narrow, setNarrow] = createSignal(window.innerWidth <= 800);
  onMount(() => {
    const update = (): void => {
      setNarrow(window.innerWidth <= 800);
    };
    window.addEventListener("resize", update);
    onCleanup(() => window.removeEventListener("resize", update));
  });
  const [marked, setMarked] = createSignal<string | null>(termUrl.read());
  // Retain a deep-linked key until its list row or direct lookup resolves.
  const [fromAddress, setFromAddress] = createSignal<string | null>(
    termUrl.read(),
  );

  createEffect(
    on(
      mode,
      () => {
        const named = termUrl.read();
        setMarked(named);
        setFromAddress(named);
        setDetailOpen(named !== null);
      },
      { defer: true },
    ),
  );

  const browsed = createReadingResource(
    () => (mode() === "browse" ? (search.term() ?? "") : false),
    (term) =>
      term === ""
        ? client.glossaryTerms({ limit: TERM_LIMIT })
        : client.searchGlossary(term, { limit: TERM_LIMIT }),
  );
  const due = createReadingResource(
    () => (mode() === "study" ? (search.term() ?? "") : false),
    (term) =>
      client.glossaryDue({
        query: term === "" ? undefined : term,
        limit: TERM_LIMIT,
      }),
  );
  const [unfilteredDueTotal, setUnfilteredDueTotal] = createSignal<
    number | null
  >(null);
  createEffect(() => {
    if (mode() === "study" && search.term() === null) {
      const result = due.ready();
      if (typeof result?.total === "number") {
        setUnfilteredDueTotal(result.total);
      }
    }
  });

  const page = (): GlossaryTermsResult | undefined =>
    mode() === "study" ? due.ready() : browsed.ready();

  const [appended, setAppended] = createSignal<NodeRecord[]>([]);
  const [position, setPosition] = createSignal<string | null | undefined>(
    undefined,
  );
  const [continuingListing, setContinuingListing] = createSignal<string | null>(
    null,
  );
  const [pageFailure, setPageFailure] = createSignal<unknown>(undefined);
  // Suppress automatic retry storms; explicit retry remains available.
  const [failedPosition, setFailedPosition] = createSignal<string | null>(null);

  const listingKey = createMemo(() =>
    `${mode()}:${search.term() ?? ""}`,
  );

  const startPagesOver = (): void => {
    setAppended([]);
    setPosition(undefined);
    setPageFailure(undefined);
    setFailedPosition(null);
  };

  createEffect(on(listingKey, startPagesOver, { defer: true }));

  // Keep the last successful first page visible during a failed refresh.
  const [keptPage, setKeptPage] = createSignal<KeptPage | null>(null);

  // A changed first-page boundary invalidates every appended page.
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

  const firstRows = (): NodeRecord[] => {
    const answered = page();
    if (answered !== undefined) {
      return answered.terms;
    }
    const kept = keptPage();
    return kept !== null && kept.listing === listingKey() ? kept.rows : [];
  };

  const held = createMemo<NodeRecord[]>(() =>
    firstOfEachKey([...firstRows(), ...appended()]),
  );

  const loading = (): boolean =>
    mode() === "study" ? due.loading() : browsed.loading();
  const failure = (): unknown =>
    mode() === "study" ? due.error() : browsed.error();

  createEffect(() => {
    const key = fromAddress();
    if (key !== null && held().some((row) => row.node_key === key)) {
      setFromAddress(null);
    }
  });

  const addressUnheld = (): string | false => {
    const key = fromAddress();
    if (key === null || page() === undefined) {
      return false;
    }
    return held().some((row) => row.node_key === key) ? false : key;
  };

  const addressed = createReadingResource(addressUnheld, (key) =>
    client.glossaryTerm(key),
  );

  const offList = (): NodeRecord | undefined =>
    fromAddress() === null ? undefined : (addressed.ready()?.term ?? undefined);
  const terms = createMemo<NodeRecord[]>(() => {
    const addressedTerm = offList();
    return addressedTerm === undefined
      ? held()
      : firstOfEachKey([addressedTerm, ...held()]);
  });

  createEffect(() => {
    if (
      narrow() &&
      page() !== undefined &&
      !loading() &&
      failure() === undefined &&
      terms().length === 0
    ) {
      setDetailOpen(true);
    }
  });

  // Ranked browse search has no stable cursor; dictionary and due listings do.
  const continuable = (): boolean =>
    mode() === "study" || search.term() === null;

  const nextPosition = (): string | null => {
    const advanced = position();
    if (advanced !== undefined) {
      return advanced;
    }
    const first = page();
    return first?.has_more ? (first.next_position ?? null) : null;
  };

  const moreFollow = (): boolean =>
    continuable() ? nextPosition() !== null : (page()?.has_more ?? false);

  const canContinue = (): boolean => continuable() && moreFollow();

  const continuing = (): boolean => continuingListing() === listingKey();

  const cutStatement = (): string | null => {
    const total = page()?.total;
    if (typeof total !== "number" || !moreFollow()) {
      return null;
    }
    return continuable()
      ? `${held().length} of ${total} terms read.`
      : `${held().length} of ${total} matches shown; a narrower search reaches the rest.`;
  };

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
          ? await client.glossaryDue({
              query: search.term() ?? undefined,
              limit: TERM_LIMIT,
              after,
            })
          : await client.glossaryTerms({ limit: TERM_LIMIT, after });
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

  const continueOnScroll = (): void => {
    if (nextPosition() !== failedPosition()) {
      void continueListing();
    }
  };

  const emptyMessage = (): string => {
    if (mode() === "study") {
      return search.term() !== null && unfilteredDueTotal() !== 0
        ? "No terms due for review match that filter."
        : "Nothing is due for review.";
    }
    return search.term() === null
      ? "The glossary has no terms yet."
      : "No terms match that search.";
  };

  const dueSummary = (): string | null => {
    const listing = due.ready();
    if (mode() !== "study" || !listing || !(listing.total > 0)) {
      return null;
    }
    if (search.term() !== null) {
      const matches = listing.total === 1 ? "1 due term matches" : `${listing.total} due terms match`;
      return `${matches}, in schedule order.`;
    }
    const counted = listing.total === 1 ? "1 term is" : `${listing.total} terms are`;
    return `${counted} due, in schedule order: never reviewed first, then by due date, then by file path.`;
  };

  const fieldLabel = (): string =>
    mode() === "study"
      ? "Search due terms"
      : "Search the glossary";

  useDocumentTitle(() =>
    mode() === "study" ? "Due terms" : "Glossary",
  );

  const listboxId = "glossary-terms";
  const optionId = (index: number): string => `${OPTION_ID}-${index}`;

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

  const hover = createHoverMotion();

  const markRow = (index: number | null): void => {
    const key = index === null ? null : (terms()[index]?.node_key ?? null);
    setMarked(key);
    if (key !== fromAddress()) {
      setFromAddress(null);
    }
    termUrl.replace(key);
  };

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

  const selected = (): NodeRecord | undefined => {
    const index = active();
    return index === null ? undefined : terms()[index];
  };

  let detail!: HTMLDivElement;
  createEffect(
    on(
      () => selected()?.node_key,
      () => {
        if (detail) {
          detail.scrollTop = 0;
        }
      },
      { defer: true },
    ),
  );

  const offListStatement = (): string | null => {
    const off = offList();
    return off === undefined
      ? null
      : `${off.title} was opened from its link and is outside the current page.`;
  };

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

  createEffect(() => {
    revealOption(activeId());
  });

  const FILTERS: readonly { id: GlossaryMode; label: string }[] = [
    { id: "browse", label: "All terms" },
    { id: "study", label: "Due terms" },
  ];

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
    <main
      class="glossary"
      classList={{ "glossary--detail-open": detailOpen() }}
    >
      <div class="glossary-list">
        <h1 class="glossary-title">Glossary</h1>
        <div class="glossary-modes" role="group" aria-label="Glossary listing">
          <For each={FILTERS}>
            {(filter) => (
              <button
                type="button"
                aria-pressed={mode() === filter.id}
                aria-controls={terms().length > 0 ? listboxId : undefined}
                class="glossary-mode"
                onClick={() => show(filter.id)}
              >
                {filter.label}
              </button>
            )}
          </For>
        </div>

        <div class="glossary-search-row">
          <input
            type="search"
            class="glossary-search"
            placeholder={fieldLabel()}
            autocomplete="off"
            aria-label={fieldLabel()}
            role="combobox"
            aria-expanded={terms().length > 0}
            aria-controls={terms().length > 0 ? listboxId : undefined}
            aria-activedescendant={activeId()}
            value={search.query()}
            onInput={(event) => search.input(event.currentTarget.value)}
            onKeyDown={onKeyDown}
          />
          <Show when={search.query().length > 0}>
            <button
              type="button"
              class="glossary-search__clear"
              onClick={() => search.clear()}
            >
              Clear
            </button>
          </Show>
        </div>
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
          <ul
            id={listboxId}
            role="listbox"
            aria-label="Glossary terms"
            class="glossary-terms"
            onMouseLeave={hover.left}
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
                  onMouseEnter={(event) => {
                    if (hover.crossed({ x: event.clientX, y: event.clientY })) {
                      markRow(index());
                    }
                  }}
                  onMouseMove={(event) => {
                    hover.moved({ x: event.clientX, y: event.clientY });
                  }}
                  onClick={() => {
                    markRow(index());
                    setDetailOpen(true);
                  }}
                >
                  <span class="glossary-term__title">{term.title}</span>
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

      <div ref={detail} class="glossary-detail">
        <div class="glossary-detail__toolbar">
          <button
            type="button"
            class="glossary-detail__back"
            onClick={() => setDetailOpen(false)}
          >
            Back to terms
          </button>
          <Show when={narrow()}>
            <h1 class="glossary-detail__title">Glossary</h1>
          </Show>
        </div>
        <Show
          when={selected()}
          fallback={
            <Show when={!loading() && !failure() && !addressed.loading()}>
              <Show
                when={terms().length > 0}
                fallback={
                  <EmptyGlossary
                    mode={mode()}
                    searched={search.term() !== null}
                    filtered={
                      mode() === "study" &&
                      search.term() !== null &&
                      unfilteredDueTotal() !== 0
                    }
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
