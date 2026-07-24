/*
 * One reading column: a single note, fetched, parsed, and rendered.
 *
 * The column owns its own focus-aware resource keyed on a note *reference* —
 * either an `id:<uuid>` link target or a raw slipbox key (see `fetchNoteContext`
 * for how an id is resolved before the body is read).
 *
 * The column renders in one of the spine's presentation states. When `obscured`
 * it collapses to a vertical title sliver drawn from the loaded note; the sliver
 * is a real button, so a scrolled-away column can be brought back with a click
 * or the keyboard (`onReveal`) instead of being a dead, `aria-hidden` label.
 * Otherwise it renders the title as the column's `h1` chrome and the parsed Org
 * body beneath. Links inside the body speak the navigation grammar
 * (glance/pin/go); this column supplies the `Navigation` the spine builds for
 * its stack position.
 */

import { Show, createMemo, type Component } from "solid-js";

import { ApiError } from "../api/client.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { NavigationProvider, type Navigation } from "../org/navigation.jsx";
import { parseOrg } from "../org/parse.js";
import { RenderDocument } from "../org/RenderDocument.jsx";
import { fetchNoteContext, WHOLE_NOTE_MAX_LINES } from "./fetch-note.js";
import type { ColumnState } from "./spine-geometry.js";

function describeError(error: unknown): string {
  if (error instanceof ApiError) {
    return error.isNotFound ? "This note is not in the slipbox." : error.message;
  }
  return "This note could not be read.";
}

export const ReadingColumn: Component<{
  reference: string;
  state: ColumnState;
  navigation: Navigation;
  /** Bring this column back into view from its collapsed sliver. */
  onReveal: () => void;
}> = (props) => {
  const context = createReadingResource(
    () => props.reference,
    (reference) => fetchNoteContext(reference, WHOLE_NOTE_MAX_LINES),
  );

  const document = createMemo(() => {
    const value = context.ready();
    return value ? parseOrg(value.source.content) : null;
  });

  const title = (): string => context.ready()?.note.title ?? "…";

  return (
    <Show when={props.state === "obscured"} fallback={
      <article class="reading-note">
        <Show
          when={context.error()}
          fallback={
            <Show
              when={context.ready()}
              fallback={<p class="reading-note__status">Reading…</p>}
            >
              {(ready) => (
                <>
                  <header class="reading-note__header">
                    <h1 class="reading-note__title">{ready().note.title}</h1>
                  </header>
                  <NavigationProvider navigation={props.navigation}>
                    <Show when={document()}>
                      {(parsed) => <RenderDocument document={parsed()} />}
                    </Show>
                  </NavigationProvider>
                </>
              )}
            </Show>
          }
        >
          {(error) => (
            <p class="reading-note__status reading-note__status--error">
              {describeError(error())}
            </p>
          )}
        </Show>
      </article>
    }>
      <button
        type="button"
        class="reading-note reading-note--obscured"
        onClick={() => props.onReveal()}
        aria-label={`Reveal ${title()}`}
      >
        <span class="reading-note__sliver-label">{title()}</span>
      </button>
    </Show>
  );
};
