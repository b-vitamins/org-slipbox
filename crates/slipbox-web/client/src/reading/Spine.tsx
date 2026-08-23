/*
 * The reading spine: a horizontal stack of note columns. Owns the DOM wiring
 * only; states come from `spine-geometry` and the navigation verbs from
 * `spine-navigation`. Each column and the preview card sits behind its own error
 * boundary, since a caught Solid error latches until reset.
 */

import {
  ErrorBoundary,
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type Component,
} from "solid-js";

import { createReadingResource } from "../data/create-reading-resource.js";
import { scrollBehavior } from "../dom/reduced-motion.js";
import { useDocumentTitle } from "../dom/document-title.js";
import { referenceOf } from "../org/navigation.jsx";
import { createGlanceController } from "./glance-controller.js";
import { GlancePreview } from "./GlancePreview.jsx";
import { ReadingColumn } from "./ReadingColumn.jsx";
import {
  frontmostReference,
  resolveNoteTitle,
  unresolvedTitle,
} from "./reading-title.js";
import { spineNavigation } from "./spine-navigation.js";
import {
  columnOffset,
  columnStates,
  isPinned,
  scrollTargetFor,
  verticalRevealedColumn,
  verticalRevealTop,
  type ColumnState,
  type SpineMetrics,
} from "./spine-geometry.js";
import type { ReadingStack } from "./stack.js";
import "./reading.css";

/**
 * What a column shows in place of a note its renderer could not draw.
 *
 * `retry` must be the boundary's own reset: a caught error latches until then,
 * and resetting rebuilds the column, which re-reads the note.
 */
const UnreadableColumn: Component<{ retry: () => void }> = (props) => (
  <article class="reading-note">
    <p class="reading-note__status reading-note__status--error">
      This note could not be rendered.
    </p>
    <button
      type="button"
      class="reading-note__retry"
      onClick={() => props.retry()}
    >
      Read it again
    </button>
  </article>
);

function readPixelToken(element: HTMLElement, name: string, fallback: number): number {
  const raw = getComputedStyle(element).getPropertyValue(name).trim();
  const parsed = Number.parseFloat(raw);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export const Spine: Component<{ stack: ReadingStack }> = (props) => {
  let container!: HTMLDivElement;
  const [scrollLeft, setScrollLeft] = createSignal(0);
  const [metrics, setMetrics] = createSignal<SpineMetrics>({
    columnWidth: 625,
    sliver: 40,
    viewport: 0,
    scrollWidth: 0,
  });

  // Read from the computed `flex-direction` rather than a duplicated breakpoint,
  // so reading.css stays the single source of the narrow-layout media query.
  const [narrow, setNarrow] = createSignal(false);
  const [activeIndex, setActiveIndex] = createSignal(
    Math.max(0, props.stack.keys().length - 1),
  );

  const glances = createGlanceController();
  onCleanup(() => glances.cancel());

  // Handle of the reveal frame in flight, so a rapid re-pin supersedes it rather
  // than two chains fighting over the scroll offset.
  let revealFrame: number | null = null;
  const cancelReveal = (): void => {
    if (revealFrame !== null) {
      cancelAnimationFrame(revealFrame);
      revealFrame = null;
    }
  };
  onCleanup(cancelReveal);

  // The tab is titled after the frontmost column. While the resolve is in
  // flight the accessor returns undefined, so `useDocumentTitle` falls back to
  // the product name rather than holding a stale title.
  const frontmost = createMemo(() => frontmostReference(props.stack.keys()));
  const focusTitle = createReadingResource(frontmost, resolveNoteTitle);
  useDocumentTitle(() =>
    focusTitle.error() ? unresolvedTitle(focusTitle.error()) : focusTitle.ready(),
  );

  // Returns the same snapshot it publishes, so `revealColumn` can read the fresh
  // geometry before Solid flushes the signal write into `metrics()`.
  const measure = (): SpineMetrics => {
    const next: SpineMetrics = {
      columnWidth: readPixelToken(container, "--column-width", 625),
      sliver: readPixelToken(container, "--column-sliver", 40),
      viewport: container.clientWidth,
      scrollWidth: container.scrollWidth,
    };
    setMetrics(next);
    setNarrow(getComputedStyle(container).flexDirection === "column");
    return next;
  };

  const stackedIndex = (): number | undefined =>
    verticalRevealedColumn(
      container.getBoundingClientRect().top,
      [...container.querySelectorAll(".spine-column")].map(
        (column) => column.getBoundingClientRect().top,
      ),
    );

  const horizontalIndex = (left: number, snapshot: SpineMetrics): number => {
    const count = props.stack.keys().length;
    if (count === 0) {
      return 0;
    }
    if (snapshot.scrollWidth <= snapshot.viewport) {
      return count - 1;
    }
    for (let index = 0; index < count; index += 1) {
      if (!isPinned(index, left, snapshot)) {
        return index;
      }
    }
    return count - 1;
  };

  onMount(() => {
    measure();
    const onResize = (): void => {
      const wasNarrow = narrow();
      const index = activeIndex();
      measure();
      if (wasNarrow !== narrow()) {
        revealColumn(index);
      }
    };
    window.addEventListener("resize", onResize);
    onCleanup(() => window.removeEventListener("resize", onResize));
  });

  // Scroll column `index` into view. The rAF defers the measure until the freshly
  // pinned column is laid out. The narrow layout stacks vertically, so it scrolls
  // by the live distance between the two boxes (see `verticalRevealTop`).
  const revealColumn = (index: number): void => {
    cancelReveal();
    setActiveIndex(index);
    const behavior = scrollBehavior();
    revealFrame = requestAnimationFrame(() => {
      revealFrame = null;
      const snapshot = measure();
      if (narrow()) {
        const column = container.querySelectorAll(".spine-column").item(index);
        if (column instanceof HTMLElement) {
          container.scrollTo({
            top: verticalRevealTop(
              {
                top: container.getBoundingClientRect().top,
                scrollTop: container.scrollTop,
              },
              column.getBoundingClientRect().top,
            ),
            behavior,
          });
        }
        return;
      }
      const target = scrollTargetFor(index, snapshot);
      if (target !== null) {
        container.scrollTo({ left: target, behavior });
      }
    });
  };

  // Tracks the key list, not its length, so a stack that changed without growing
  // (a mid-stack pin replacing the columns to its right) still reveals.
  createEffect(() => {
    const keys = props.stack.keys();
    revealColumn(keys.length - 1);
  });

  // A card is positioned against the link that raised it, so any scroll dismisses
  // it. `scroll` does not bubble, so a column's own vertical scroll reaches the
  // spine only on the capture phase.
  const onScroll = (event: Event): void => {
    if (event.target === container) {
      const layoutIsNarrow =
        getComputedStyle(container).flexDirection === "column";
      if (layoutIsNarrow === narrow() && layoutIsNarrow) {
        const index = stackedIndex();
        if (index !== undefined) {
          setActiveIndex(index);
        }
      } else if (layoutIsNarrow === narrow()) {
        const left = container.scrollLeft;
        setScrollLeft(left);
        setActiveIndex(horizontalIndex(left, metrics()));
      }
    }
    glances.glance(null);
  };

  onMount(() => {
    container.addEventListener("scroll", onScroll, true);
    onCleanup(() => container.removeEventListener("scroll", onScroll, true));
  });

  // The offset a column pins at, written twice: as the column's own `sticky`
  // offset, and as the scroll margin of the snap mark standing in for it. CSS
  // cannot compute a per-index offset, so both are written here.
  const pin = (index: number): string => `${columnOffset(index, metrics())}px`;

  const states = createMemo<ColumnState[]>(() =>
    columnStates(props.stack.keys().length, scrollLeft(), metrics(), narrow()),
  );

  return (
    <main ref={container} class="spine">
      <For each={props.stack.keys()}>
        {(reference, index) => (
          <>
            {/* Where this column comes to rest, marked for the scrollport at the
                column's own place in the flow. The column cannot carry the mark
                itself: it is `sticky`, and a pinned box takes its snap position
                with it (see reading.css). */}
            <div
              class="spine-snap"
              aria-hidden="true"
              style={{ "scroll-margin-left": pin(index()) }}
            />
            <section
              class="spine-column"
              classList={{
                "spine-column--resting": (states()[index()] ?? "resting") === "resting",
                "spine-column--overlay": states()[index()] === "overlay",
                "spine-column--obscured": states()[index()] === "obscured",
              }}
              style={{ left: pin(index()) }}
            >
              <nav class="spine-position" aria-label="Reading trail">
                <button
                  type="button"
                  class="spine-position__move"
                  disabled={index() === 0}
                  onClick={() => revealColumn(index() - 1)}
                >
                  Previous
                </button>
                <span
                  class="spine-position__count"
                  aria-current={activeIndex() === index() ? "step" : undefined}
                >
                  Note {index() + 1} of {props.stack.keys().length}
                </span>
                <button
                  type="button"
                  class="spine-position__move"
                  disabled={index() + 1 === props.stack.keys().length}
                  onClick={() => revealColumn(index() + 1)}
                >
                  Next
                </button>
              </nav>
              {/* The boundary must sit outside the column, not inside it: a Solid
                  boundary cannot catch a throw from the scope it is rendered in.
                  The fallback is handed the boundary's own reset, since a caught
                  error latches and nothing the column fetches later clears it. */}
              <ErrorBoundary
                fallback={(_error, reset) => <UnreadableColumn retry={reset} />}
              >
                <ReadingColumn
                  reference={reference}
                  state={states()[index()] ?? "resting"}
                  navigation={spineNavigation(props.stack, glances, index, revealColumn)}
                  onReveal={() => revealColumn(index())}
                />
              </ErrorBoundary>
            </section>
          </>
        )}
      </For>
      <Show when={glances.request()}>
        {(request) => (
          // Keyed to the target: a caught error latches until the boundary is
          // discarded, and moving between links swaps an open card in place, so
          // one shared boundary would silence every later preview. The fallback
          // must take the error, since Solid logs a stack for any fallback that
          // does not.
          <Show when={referenceOf(request().target)} keyed>
            <ErrorBoundary fallback={(_error) => null}>
              <GlancePreview request={request()} />
            </ErrorBoundary>
          </Show>
        )}
      </Show>
    </main>
  );
};
