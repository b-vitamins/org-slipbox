/*
 * One reading column, keyed on a note reference (`id:<uuid>` or a slipbox key),
 * rendered in one of the spine's `ColumnState`s; an obscured column hides its
 * note and shows a title sliver. The body is a slice, not a file. The server
 * caps every source read at `WHOLE_NOTE_MAX_LINES`.
 */

import {
  Show,
  createEffect,
  createMemo,
  createUniqueId,
  type Component,
} from "solid-js";

import { ApiError } from "../api/client.js";
import type { NoteContext, NotePlace } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { encodeQuery } from "../entry/query-url.js";
import { NavigationProvider, type Navigation } from "../org/navigation.jsx";
import { parseOrg } from "../org/parse.js";
import { RenderDocument } from "../org/RenderDocument.jsx";
import { fetchNoteContext, WHOLE_NOTE_MAX_LINES } from "./fetch-note.js";
import type { ColumnTitle } from "./reading-title.js";
import { RelationsFooter } from "./RelationsFooter.jsx";
import type { ColumnState } from "./spine-geometry.js";
import type { FilingMove } from "./spine-navigation.js";

function describeError(error: unknown): string {
  if (error instanceof ApiError) {
    return error.isNotFound
      ? "This note is not in the slipbox."
      : error.message;
  }
  return "This note could not be read.";
}

/** Whether the read failed because the slipbox holds no such note. */
function isMissing(error: unknown): boolean {
  return error instanceof ApiError && error.isNotFound;
}

/**
 * The name a note reference carries, as words to search for, or `null` when it
 * carries none. An `id:` reference is a uuid, which matches no prose, so it
 * yields null; a key's filename stem is the note's name.
 */
function searchableName(reference: string): string | null {
  if (reference.startsWith("id:")) {
    return null;
  }
  const path = reference.replace(/^(?:file|heading):/, "").split("::")[0] ?? "";
  const stem = (path.split("/").pop() ?? "").replace(/\.org$/, "");
  const words = stem.replace(/[-_]+/g, " ").trim();
  return words === "" ? null : words;
}

/**
 * How much of the note the slice holds, or `null` when it holds all of it.
 *
 * Compares `source.line_count` against `node_line_count`; both measure the note.
 * `total_lines` measures the file, which for a heading note also counts every
 * sibling note in it. `truncated_before` is fixed false for a node read, whose
 * window begins at the note's first line, so only `truncated_after` is read.
 */
function shortfall(context: NoteContext): string | null {
  if (!context.source.truncated_after) {
    return null;
  }
  const shown = context.source.line_count;
  const noteLines = context.node_line_count;
  return `Showing ${shown} of this note's ${noteLines} lines.`;
}

function filedAt(place: NotePlace | undefined): string | null {
  return place === undefined ? null : `Filed ${place.ordinal} of ${place.total}`;
}

export const ReadingColumn: Component<{
  reference: string;
  state: ColumnState;
  navigation: Navigation;
  readOn: FilingMove;
  /** Bring this column back into view from its collapsed sliver. */
  onReveal: () => void;
  onTitle?: (known: ColumnTitle) => void;
}> = (props) => {
  const context = createReadingResource(
    () => props.reference,
    (reference) => fetchNoteContext(reference, WHOLE_NOTE_MAX_LINES),
  );

  createEffect(() => {
    const value = context.ready();
    const failure = context.error();
    if (value !== undefined) {
      props.onTitle?.({ title: value.note.title });
    } else if (failure !== undefined) {
      props.onTitle?.({ error: failure });
    }
  });

  const document = createMemo(() => {
    const value = context.ready();
    return value ? parseOrg(value.source.content) : null;
  });

  const title = (): string => context.ready()?.note.title ?? "…";

  const obscured = (): boolean => props.state === "obscured";

  const searchFor = (): string | null =>
    isMissing(context.error()) ? searchableName(props.reference) : null;

  // One id per mounted column: a spine holds several at once, and a shared id
  // would aim every column's `aria-labelledby` at the first one's heading.
  const headingId = createUniqueId();

  return (
    <>
      {/* `hidden` rather than unmounting: it removes the note from layout and
          from the accessibility tree while keeping the reader's state inside it.
          Every state carries `headingId` on whatever element heads it. */}
      <article
        class="reading-note"
        tabindex="-1"
        hidden={obscured()}
        aria-labelledby={headingId}
      >
        <Show
          when={context.ready()}
          fallback={
            <>
              {/* One element for both the reading and failed states: a live
                  region announces nothing about content it already held when it
                  was inserted, so the failure must land in a region that is
                  already placed. An element carries one role, and `heading` is
                  the one the outline needs, so the announcement comes from the
                  global `aria-live` attribute instead of `role="status"`. */}
              <p
                id={headingId}
                class="reading-note__status"
                classList={{
                  "reading-note__status--error": context.error() !== undefined,
                }}
                role="heading"
                aria-level="1"
                aria-live="polite"
              >
                {context.error() === undefined
                  ? "Reading…"
                  : describeError(context.error())}
              </p>
              <Show when={context.error() !== undefined}>
                <p class="reading-note__reference">{props.reference}</p>
                <div class="reading-note__ways-out">
                  <button
                    type="button"
                    class="reading-note__retry"
                    onClick={() => context.refetch()}
                  >
                    Try reading it again
                  </button>
                  {/* Query-only href, so it resolves against whatever path the
                      surface is served under rather than the site root. */}
                  <Show when={searchFor()}>
                    {(term) => (
                      <a
                        class="reading-note__search"
                        href={encodeQuery(term(), "")}
                      >
                        Search the slipbox for {term()}
                      </a>
                    )}
                  </Show>
                </div>
              </Show>
            </>
          }
        >
          {(ready) => (
            <NavigationProvider navigation={props.navigation}>
              <header class="reading-note__header">
                <h1 id={headingId} class="reading-note__title">
                  {ready().note.title}
                </h1>
                <Show when={filedAt(ready().place)}>
                  {(line) => <p class="reading-note__place">{line()}</p>}
                </Show>
              </header>
              <Show when={document()}>
                {(parsed) => <RenderDocument document={parsed()} />}
              </Show>
              <Show when={shortfall(ready())}>
                {(notice) => <p class="reading-note__truncated">{notice()}</p>}
              </Show>
              <RelationsFooter context={ready()} readOn={props.readOn} />
            </NavigationProvider>
          )}
        </Show>
      </article>
      <Show when={obscured()}>
        <button
          type="button"
          class="reading-note reading-note--obscured"
          onClick={() => props.onReveal()}
          aria-label={`Reveal ${title()}`}
        >
          <span class="reading-note__sliver-label">{title()}</span>
        </button>
      </Show>
    </>
  );
};
