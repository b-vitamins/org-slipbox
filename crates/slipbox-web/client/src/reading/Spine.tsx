/*
 * The reading spine: a horizontal stack of note columns.
 *
 * The spine renders one column per reference in the reading stack and drives
 * the near/far presentation: as the reader scrolls right, earlier columns
 * collapse to a titled sliver (`obscured`) or float pinned over their neighbors
 * (`overlay`). Column states are recomputed from the live scroll offset via the
 * pure `spine-geometry` helpers; this component owns only the DOM wiring —
 * measuring the container, tracking `scrollLeft`, and smooth-scrolling a
 * freshly opened column into view.
 *
 * Following a link is routed from a column into the stack reducer: a click in
 * column `n` prunes the columns to its right and appends the target, and the
 * URL updates as a side effect of the stack commit. The reducer answers with the
 * column the target landed in — a fresh one, or the one already holding that
 * note — and the spine scrolls there, so a follow always ends with its note in
 * view whether or not it opened a column.
 *
 * Each column is rendered behind its own error boundary. A column parses and
 * renders a note the reader did not write and cannot repair, and a note the
 * renderer cannot draw throws out of the update that resolved it — where nothing
 * else is listening. Bounding each column separately turns that into one column
 * reporting itself unreadable, rather than a column that keeps saying it is
 * still reading a note it will never draw. A caught error holds until its
 * boundary is reset, so the boundary offers the reader a way to reset it: a
 * retry button in the column that could not be drawn.
 */

import {
  ErrorBoundary,
  For,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type Component,
} from "solid-js";

import { ReadingColumn } from "./ReadingColumn.jsx";
import {
  columnOffset,
  columnState,
  scrollTargetFor,
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

  const measure = (): void => {
    setMetrics({
      columnWidth: readPixelToken(container, "--column-width", 625),
      sliver: readPixelToken(container, "--column-sliver", 40),
      viewport: container.clientWidth,
      scrollWidth: container.scrollWidth,
    });
  };

  onMount(() => {
    measure();
    const onResize = (): void => measure();
    window.addEventListener("resize", onResize);
    onCleanup(() => window.removeEventListener("resize", onResize));
  });

  // Scroll column `index` into view. The microtask defers the measure until the
  // freshly opened column is laid out, so the scroll target is computed from the
  // new scrollable width rather than the previous one.
  const revealColumn = (index: number): void => {
    queueMicrotask(() => {
      measure();
      const target = scrollTargetFor(index, metrics());
      if (target !== null) {
        container.scrollTo({ left: target, behavior: "smooth" });
      }
    });
  };

  // Whenever the stack changes — mounted from a URL, or restored by back and
  // forward — bring its frontmost column into view.
  createEffect(() => {
    const keys = props.stack.keys();
    revealColumn(keys.length - 1);
  });

  const onScroll = (): void => {
    setScrollLeft(container.scrollLeft);
  };

  const states = createMemo<ColumnState[]>(() => {
    const keys = props.stack.keys();
    return keys.map((_, index) =>
      columnState(index, keys.length, scrollLeft(), metrics()),
    );
  });

  return (
    <div ref={container} class="spine" onScroll={onScroll}>
      <For each={props.stack.keys()}>
        {(reference, index) => (
          <section
            class="spine-column"
            classList={{
              "spine-column--resting": (states()[index()] ?? "resting") === "resting",
              "spine-column--overlay": states()[index()] === "overlay",
              "spine-column--obscured": states()[index()] === "obscured",
            }}
            style={{ left: `${columnOffset(index(), metrics())}px` }}
          >
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
                onFollow={(target) => revealColumn(props.stack.follow(index(), target))}
              />
            </ErrorBoundary>
          </section>
        )}
      </For>
    </div>
  );
};
