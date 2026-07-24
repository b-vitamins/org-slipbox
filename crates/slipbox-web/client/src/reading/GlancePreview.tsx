/*
 * The glance preview card.
 *
 * A floating, read-only peek at a link's target: its title and a one-line
 * excerpt of its opening prose, fetched lazily the first time the reader lingers
 * on a link. The excerpt is rendered, not spelled out — a note whose first
 * sentence carries a formula previews with that formula typeset, since a peek at
 * a note should look like the note. The card is decorative chrome — the link
 * itself stays the accessible, actionable element — so it is `aria-hidden` and
 * never traps focus. It positions itself with fixed coordinates from
 * `placeGlance`, measuring its own box after mount so the flip-above and
 * edge-clamp use the real rendered size.
 */

import { Show, createEffect, createMemo, createSignal, type Component } from "solid-js";

import { ApiError } from "../api/client.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { documentPreview } from "../org/prose.js";
import { parseOrg } from "../org/parse.js";
import type { GlanceRequest } from "../org/navigation.jsx";
import { referenceOf } from "../org/navigation.jsx";
import { RenderPreview } from "../org/RenderPreview.jsx";
import { fetchNoteContext, PREVIEW_MAX_LINES } from "./fetch-note.js";
import { placeGlance, type GlancePlacement } from "./glance-position.js";

/** Characters of body text a preview shows before eliding. */
const EXCERPT_CHARS = 220;

/**
 * The one line a card shows in place of a peek it could not read.
 *
 * The server's message is not repeated: it is sized for a page, and reflowing it
 * into a floating box would invalidate the card's own measured placement.
 */
function describeError(error: unknown): string {
  return error instanceof ApiError && error.isNotFound
    ? "This note is not in the slipbox."
    : "This note could not be read.";
}

export const GlancePreview: Component<{ request: GlanceRequest }> = (props) => {
  let card!: HTMLDivElement;
  const [placement, setPlacement] = createSignal<GlancePlacement | null>(null);

  const reference = (): string => referenceOf(props.request.target);
  const context = createReadingResource(reference, (ref) =>
    fetchNoteContext(ref, PREVIEW_MAX_LINES),
  );

  const excerpt = createMemo(() => {
    const value = context.ready();
    return value ? documentPreview(parseOrg(value.source.content), EXCERPT_CHARS) : [];
  });

  // Re-place whenever the card's measured size can change, so the flip-above and
  // edge-clamp read the rendered box rather than the loading one.
  createEffect(() => {
    // Read the resolved content so Solid tracks it and re-runs after the fetch.
    void context.ready();
    void excerpt();
    void context.error();
    const anchor = props.request.origin.getBoundingClientRect();
    setPlacement(
      placeGlance(
        anchor,
        { width: card.offsetWidth, height: card.offsetHeight },
        { width: window.innerWidth, height: window.innerHeight },
      ),
    );
  });

  return (
    <div
      ref={card}
      class="glance-card"
      classList={{
        "glance-card--above": placement()?.above ?? false,
        "glance-card--placed": placement() !== null,
      }}
      style={{
        left: `${placement()?.left ?? 0}px`,
        top: `${placement()?.top ?? 0}px`,
      }}
      aria-hidden="true"
    >
      <Show
        when={context.ready()}
        fallback={
          <Show
            when={context.error()}
            fallback={<p class="glance-card__status">Reading…</p>}
          >
            {(error) => (
              <p class="glance-card__status">{describeError(error())}</p>
            )}
          </Show>
        }
      >
        {(ready) => (
          <>
            <h2 class="glance-card__title">{ready().note.title}</h2>
            <Show when={excerpt().length > 0}>
              <p class="glance-card__excerpt">
                <RenderPreview prose={excerpt()} class="glance-card__prose" />
              </p>
            </Show>
            {/* Surface the otherwise-invisible navigation grammar at the moment
             * the reader is poised over a link. */}
            <p class="glance-card__hint">Click to open · Alt-click to replace</p>
          </>
        )}
      </Show>
    </div>
  );
};
