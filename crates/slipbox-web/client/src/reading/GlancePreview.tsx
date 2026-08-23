/*
 * The glance preview card: a floating peek at a link's target, placed in
 * viewport coordinates by `placeGlance` and remeasured after mount. Hover cards
 * are decorative; keyboard cards describe their link; touch cards expose actions.
 */

import {
  Show,
  createEffect,
  createMemo,
  createSignal,
  createUniqueId,
  onCleanup,
  type Component,
} from "solid-js";

import { ApiError } from "../api/client.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { documentPreview } from "../org/prose.js";
import { parseOrg } from "../org/parse.js";
import type { GlanceRequest } from "../org/navigation.jsx";
import { referenceOf } from "../org/navigation.jsx";
import { RenderPreview } from "../org/RenderPreview.jsx";
import { fetchNoteContext, PREVIEW_MAX_LINES } from "./fetch-note.js";
import { placeGlance, type GlancePlacement, type Span } from "./glance-position.js";

/** Characters of body text a preview shows before eliding. */
const EXCERPT_CHARS = 220;

/** Measure the complete reading band so a preview never occupies another column. */
function readingBand(origin: HTMLElement, viewport: number): Span {
  let left = Number.POSITIVE_INFINITY;
  let right = Number.NEGATIVE_INFINITY;
  for (const column of origin.closest(".spine")?.querySelectorAll(".spine-column") ??
    []) {
    const box = column.getBoundingClientRect();
    left = Math.min(left, box.left);
    right = Math.max(right, box.right);
  }
  return Number.isFinite(left) ? { left, right } : { left: 0, right: viewport };
}

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
  const cardId = createUniqueId();
  const [placement, setPlacement] = createSignal<GlancePlacement | null>(null);
  let preferredWidth: number | null = null;

  const commits = (): boolean => props.request.gesture === "touch";
  const decorative = (): boolean => props.request.gesture === "hover";
  const describes = (): boolean => props.request.gesture === "focus";

  // Keep keyboard focus on the link and attach the preview as its description.
  createEffect(() => {
    if (!describes()) {
      return;
    }
    const link = props.request.origin;
    link.setAttribute("aria-describedby", cardId);
    onCleanup(() => link.removeAttribute("aria-describedby"));
  });

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
    preferredWidth ??= card.offsetWidth;
    setPlacement(
      placeGlance(
        anchor,
        { width: preferredWidth, height: card.offsetHeight },
        { width: window.innerWidth, height: window.innerHeight },
        readingBand(props.request.origin, window.innerWidth),
      ),
    );
  });

  return (
    <div
      ref={card}
      id={cardId}
      class="glance-card"
      classList={{
        "glance-card--above": placement()?.above ?? false,
        "glance-card--placed": placement() !== null,
        "glance-card--committing": commits(),
      }}
      style={{
        left: `${placement()?.left ?? 0}px`,
        top: `${placement()?.top ?? 0}px`,
        width: placement() === null ? undefined : `${placement()!.width}px`,
      }}
      aria-hidden={decorative() ? "true" : undefined}
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
            <Show
              when={commits()}
              fallback={
                <p class="glance-card__hint">Click to open · Alt-click to replace</p>
              }
            >
              <div class="glance-card__actions">
                <button
                  type="button"
                  class="glance-card__action"
                  onClick={() => props.request.pin()}
                >
                  Open
                </button>
                <button
                  type="button"
                  class="glance-card__action"
                  onClick={() => props.request.go()}
                >
                  Replace
                </button>
                {/* Touch has no pointer-out to dismiss with, so the way out is
                 * an explicit control. */}
                <button
                  type="button"
                  class="glance-card__action glance-card__action--quiet"
                  onClick={() => props.request.dismiss()}
                >
                  Close
                </button>
              </div>
            </Show>
          </>
        )}
      </Show>
    </div>
  );
};
