/*
 * One reading column: a single note, fetched, parsed, and rendered.
 *
 * The column owns its own focus-aware resource keyed on a note *reference* —
 * either an `id:<uuid>` link target or a raw slipbox key. An id reference is
 * resolved to its node first (ids survive edits that shift a key's line), then
 * the whole note body is fetched in one slice (the corpus is atomic notes, so a
 * single generous `max_lines` returns a complete body). Resolving a note also
 * records its identity, which is how the stack recognizes a link to a note some
 * column already holds under its other name.
 *
 * The column renders in one of the spine's presentation states. When `obscured`
 * it collapses to a vertical title sliver drawn from the loaded note; otherwise
 * it renders the title as the column's `h1` chrome and the parsed Org body
 * beneath. Links are followed through the navigation seam into `onFollow`, which
 * the spine binds to this column's position in the stack.
 */

import { Show, createMemo, type Component } from "solid-js";

import { client } from "../api/client.js";
import { ApiError } from "../api/client.js";
import type { NoteContext } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import {
  NavigationProvider,
  type LinkTarget,
  type Navigation,
} from "../org/navigation.jsx";
import { parseOrg } from "../org/parse.js";
import { RenderDocument } from "../org/RenderDocument.jsx";
import { noteIdentities } from "./note-identity.js";
import type { ColumnState } from "./spine-geometry.js";

/** Request enough lines to cover a whole atomic note in one slice. */
const WHOLE_NOTE_MAX_LINES = 1000;

/** Resolve a note reference (`id:<uuid>` or a key) to its reading context. */
async function fetchContext(reference: string): Promise<NoteContext> {
  const context = reference.startsWith("id:")
    ? await client
        .nodeById(reference.slice(3))
        .then((node) =>
          client.noteContext(node.node_key, { maxLines: WHOLE_NOTE_MAX_LINES }),
        )
    : await client.noteContext(reference, { maxLines: WHOLE_NOTE_MAX_LINES });
  noteIdentities.learn(context.note);
  return context;
}

function describeError(error: unknown): string {
  if (error instanceof ApiError) {
    return error.isNotFound ? "This note is not in the slipbox." : error.message;
  }
  return "This note could not be read.";
}

export const ReadingColumn: Component<{
  reference: string;
  state: ColumnState;
  onFollow: (reference: string) => void;
}> = (props) => {
  const context = createReadingResource(
    () => props.reference,
    (reference) => fetchContext(reference),
  );

  const document = createMemo(() => {
    const value = context.ready();
    return value ? parseOrg(value.source.content) : null;
  });

  const navigation: Navigation = {
    follow: (target: LinkTarget) => {
      props.onFollow(target.id ? `id:${target.id}` : target.target);
    },
  };

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
                  <NavigationProvider navigation={navigation}>
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
      <div class="reading-note reading-note--obscured" aria-hidden="true">
        <span class="reading-note__sliver-label">{title()}</span>
      </div>
    </Show>
  );
};
