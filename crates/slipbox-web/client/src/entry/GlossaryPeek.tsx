/*
 * The glossary peek pane: a selected term's definition, confirmation status, and
 * read-only SM-2 facts. Peeking a term costs no history entry; following a link
 * out of one hands its target up to be opened as a spine root, the same way
 * "Open in reader" hands up the term itself.
 */

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

/**
 * The navigation grammar as this surface can honor it. Without one, the links a
 * definition renders fall back to the inert navigation, where a click is
 * swallowed by the anchor's own `preventDefault` and commits nothing.
 *
 * The glossary is an entry surface: no spine stands beside the definition, so
 * `pin`'s "open beside its origin" and `go`'s "replace the reading path" are one
 * act here, opening the target as the reading root.
 *
 * A hovering pointer raises no preview, since the glance card is the spine's own
 * chrome. A hoverless pointer has none either, and its tap stands in for a hover
 * the device cannot make, so the tap commits: glancing into nothing would leave
 * a link no finger could follow.
 */
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
  /**
   * Open a note as a fresh reading root. The term itself is named by its slipbox
   * key; a link out of the definition is named by whichever reference the link
   * carried, which for an `id:` link is `id:<uuid>`.
   */
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

  // One navigation for the pane's whole life, reading `props.onOpen` at call time
  // rather than capturing it, so the provider beneath is never rebuilt.
  const navigation = peekNavigation((reference) => props.onOpen(reference));

  return (
    // The provider covers the header as well as the body: the way into the reader
    // is a link to the term itself, routed through the same grammar as a link out
    // of the definition, so one place decides what opening means here.
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
          {/* Beside the headword rather than under the definition, which a term
              with a long body would carry off the bottom of the pane. */}
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
      </article>
    </NavigationProvider>
  );
};
