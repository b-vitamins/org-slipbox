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
  createUniqueId,
  onCleanup,
  onMount,
  type Component,
} from "solid-js";

import { scrollBehavior } from "../dom/reduced-motion.js";
import { useDocumentTitle } from "../dom/document-title.js";
import { referenceOf } from "../org/navigation.jsx";
import { createGlanceController } from "./glance-controller.js";
import { GlancePreview } from "./GlancePreview.jsx";
import { ReadingColumn } from "./ReadingColumn.jsx";
import {
  columnTitle,
  revealedColumn,
  type ColumnTitle,
} from "./reading-title.js";
import { spineFilingMove, spineNavigation } from "./spine-navigation.js";
import {
  columnOffset,
  columnStates,
  scrollTargetFor,
  verticalRevealedColumn,
  verticalRevealTop,
  type ColumnState,
  type SpineMetrics,
} from "./spine-geometry.js";
import type { ReadingStack } from "./stack.js";
import "./reading.css";

/** Render a focusable, retryable replacement for a failed column. */
const UnreadableColumn: Component<{
  retry: () => void;
  onSettled: () => void;
}> = (props) => {
  const headingId = createUniqueId();
  onMount(() => props.onSettled());
  return (
    <article class="reading-note" tabindex="-1" aria-labelledby={headingId}>
      <p
        id={headingId}
        class="reading-note__status reading-note__status--error"
        role="heading"
        aria-level="1"
      >
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
};

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
  const [activeIndex, setActiveIndex] = createSignal(0);

  const glances = createGlanceController();
  onCleanup(() => glances.cancel());

  // Handle of the reveal frame in flight, so a rapid re-pin supersedes it rather
  // than two chains fighting over the scroll offset.
  let revealFrame: number | null = null;
  // Focus only after the reveal makes the column readable.
  const [focusWanted, setFocusWanted] = createSignal<number | null>(null);
  const cancelReveal = (): void => {
    if (revealFrame !== null) {
      cancelAnimationFrame(revealFrame);
      revealFrame = null;
    }
  };
  onCleanup(cancelReveal);

  // Reference keys prevent a replaced column from inheriting a stale title.
  const [titles, setTitles] = createSignal<ReadonlyMap<string, ColumnTitle>>(
    new Map(),
  );
  const learnTitle = (reference: string, known: ColumnTitle): void => {
    setTitles((held) => new Map(held).set(reference, known));
  };

  createEffect(() => {
    const live = new Set(props.stack.keys());
    setTitles((held) =>
      [...held.keys()].every((reference) => live.has(reference))
        ? held
        : new Map([...held].filter(([reference]) => live.has(reference))),
    );
  });

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

  onMount(() => {
    measure();
    const onResize = (): void => {
      const wasNarrow = narrow();
      const index = activeIndex();
      measure();
      if (wasNarrow !== narrow()) {
        revealColumn(index, false);
      }
    };
    window.addEventListener("resize", onResize);
    onCleanup(() => window.removeEventListener("resize", onResize));
  });

  // Scroll column `index` into view. The rAF defers the measure until the freshly
  // pinned column is laid out. The narrow layout stacks vertically, so it scrolls
  // by the live distance between the two boxes (see `verticalRevealTop`).
  const revealColumn = (index: number, focus = true): void => {
    cancelReveal();
    if (focus) {
      setFocusWanted(index);
    } else {
      setFocusWanted(null);
    }
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
        const snapshot = metrics();
        const index = revealedColumn(
          props.stack.keys().length,
          left,
          snapshot,
          snapshot.scrollWidth <= snapshot.viewport,
        );
        if (index !== undefined) {
          setActiveIndex(index);
        }
      }
    }
    glances.glance(null);
  };

  onMount(() => {
    container.addEventListener("scroll", onScroll, true);
    onCleanup(() => container.removeEventListener("scroll", onScroll, true));
  });

  // CSS cannot derive this per-column sticky and snap offset.
  const pin = (index: number): string => `${columnOffset(index, metrics())}px`;

  const states = createMemo<ColumnState[]>(() =>
    columnStates(props.stack.keys().length, scrollLeft(), metrics(), narrow()),
  );

  const entire = (): boolean => metrics().scrollWidth <= metrics().viewport;

  const revealed = createMemo(() =>
    narrow()
      ? Math.min(activeIndex(), Math.max(0, props.stack.keys().length - 1))
      : revealedColumn(
          props.stack.keys().length,
          scrollLeft(),
          metrics(),
          entire(),
        ),
  );
  useDocumentTitle(() => {
    const index = revealed();
    const reference = index === undefined ? undefined : props.stack.keys()[index];
    return reference === undefined
      ? undefined
      : columnTitle(titles().get(reference));
  });

  // Defer focus until the target is visible and its title or error has settled.
  createEffect(() => {
    const index = focusWanted();
    if (index === null || (states()[index] ?? "resting") === "obscured") {
      return;
    }
    const reference = props.stack.keys()[index];
    if (reference !== undefined && !titles().has(reference)) {
      return;
    }
    setFocusWanted(null);
    const note = container
      .querySelectorAll(".spine-column")
      .item(index)
      ?.querySelector<HTMLElement>("article.reading-note");
    note?.focus({ preventScroll: true });
  });

  return (
    <main ref={container} class="spine">
      <For each={props.stack.keys()}>
        {(reference, index) => (
          <>
            {/* Sticky columns need a separate in-flow snap target. */}
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
                  aria-current={revealed() === index() ? "step" : undefined}
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
              {/* The boundary reset is the only way to retry a latched error. */}
              <ErrorBoundary
                fallback={(error, reset) => (
                  <UnreadableColumn
                    retry={reset}
                    onSettled={() => learnTitle(reference, { error })}
                  />
                )}
              >
                <ReadingColumn
                  reference={reference}
                  state={states()[index()] ?? "resting"}
                  navigation={spineNavigation(props.stack, glances, index, revealColumn)}
                  readOn={spineFilingMove(props.stack, glances, index, revealColumn)}
                  onReveal={() => revealColumn(index())}
                  onTitle={(known) => learnTitle(reference, known)}
                />
              </ErrorBoundary>
            </section>
          </>
        )}
      </For>
      <Show when={glances.request()}>
        {(request) => (
          // Discard the latched boundary when the preview target changes.
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
