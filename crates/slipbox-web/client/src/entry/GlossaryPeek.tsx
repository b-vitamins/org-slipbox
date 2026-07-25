/*
 * The glossary peek pane: a selected term's definition, confirmation status, and
 * read-only SM-2 facts, rendered in place without touching the reading stack or
 * the URL. "Open in reader" hands the key up to be opened as a spine root.
 */

import { Show, createMemo, type Component } from "solid-js";

import { ApiError } from "../api/client.js";
import type { NodeRecord } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { parseOrg } from "../org/parse.js";
import { RenderDocument } from "../org/RenderDocument.jsx";
import { fetchNoteContext, WHOLE_NOTE_MAX_LINES } from "../reading/fetch-note.js";
import { StudyFacts } from "./StudyFacts.jsx";

function describeError(error: unknown): string {
  if (error instanceof ApiError) {
    return error.isNotFound ? "This term is not in the slipbox." : error.message;
  }
  return "This term could not be read.";
}

export const GlossaryPeek: Component<{
  term: NodeRecord;
  onOpen: (key: string) => void;
}> = (props) => {
  const context = createReadingResource(
    () => props.term.node_key,
    (key) => fetchNoteContext(key, WHOLE_NOTE_MAX_LINES),
  );

  const document = createMemo(() => {
    const value = context.ready();
    return value ? parseOrg(value.source.content) : null;
  });

  const isStub = (): boolean => props.term.glossary_status === "stub";

  return (
    <article class="glossary-peek">
      <header class="glossary-peek__header">
        <h2 class="glossary-peek__title">{props.term.title}</h2>
        <Show when={props.term.glossary_status}>
          <span
            class="glossary-peek__status"
            classList={{ "glossary-peek__status--stub": isStub() }}
          >
            {props.term.glossary_status}
          </span>
        </Show>
      </header>

      <Show
        when={context.error()}
        fallback={
          <Show
            when={context.ready()}
            fallback={<p class="glossary-peek__status-note">Reading…</p>}
          >
            <Show when={document()}>
              {(parsed) => <RenderDocument document={parsed()} />}
            </Show>
          </Show>
        }
      >
        {(error) => (
          <p class="glossary-peek__status-note glossary-peek__status-note--error">
            {describeError(error())}
          </p>
        )}
      </Show>

      <StudyFacts term={props.term} />

      <button
        type="button"
        class="glossary-peek__open"
        onClick={() => props.onOpen(props.term.node_key)}
      >
        Open in reader
      </button>
    </article>
  );
};
