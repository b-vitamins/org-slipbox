import { Show, createMemo, type Component } from "solid-js";

import { ApiError } from "../api/client.js";
import type { NodeRecord } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { GrammarLink } from "../org/GrammarLink.jsx";
import {
  NavigationProvider,
  referenceOf,
  targetForNote,
  type LinkTarget,
  type Navigation,
} from "../org/navigation.jsx";
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

// Glossary links open a new reading root; touch cannot raise a spine preview.
function peekNavigation(onOpen: (reference: string) => void): Navigation {
  const open = (target: LinkTarget): void => onOpen(referenceOf(target));
  return {
    glance: (request) => {
      if (request?.gesture === "touch") {
        request.go();
      }
    },
    pin: open,
    go: open,
  };
}

export const GlossaryPeek: Component<{
  term: NodeRecord;
  onOpen: (reference: string) => void;
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

  const navigation = peekNavigation((reference) => props.onOpen(reference));

  return (
    <NavigationProvider navigation={navigation}>
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
          <GrammarLink
            class="glossary-peek__open"
            target={targetForNote(props.term.node_key, props.term.explicit_id)}
          >
            Open in reader
          </GrammarLink>
        </header>

        <Show
          when={context.error()}
          fallback={
            <Show
              when={context.ready()}
              fallback={<p class="glossary-peek__status-note">Reading…</p>}
            >
              <Show when={document()}>
                {(parsed) => (
                  <RenderDocument document={parsed()} baseLevel={3} />
                )}
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
      </article>
    </NavigationProvider>
  );
};
