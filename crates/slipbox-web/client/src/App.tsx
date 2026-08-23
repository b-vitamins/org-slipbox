/*
 * The reading-surface shell.
 *
 * Owns the reading stack and the entry surface mode, both mirrored to the URL
 * and re-read on `popstate`, and mounts the spine, note entry, or glossary.
 */

import { Show, onCleanup, type Component } from "solid-js";

import { createColorScheme, SCHEME_LABELS } from "./dom/color-scheme.js";
import { EntrySurface } from "./entry/EntrySurface.jsx";
import { GlossaryDictionary } from "./entry/GlossaryDictionary.jsx";
import { createSurfaceView } from "./entry/surface-view.js";
import {
  browserHistory,
  onPopState,
  readEntryAddress,
  writeEntryAddress,
} from "./reading/browser-history.js";
import { Spine } from "./reading/Spine.jsx";
import { createReadingStack } from "./reading/stack.js";
import "./styles/tokens.css";
import "./App.css";

export const App: Component = () => {
  const history = browserHistory();
  const stack = createReadingStack(history);
  const view = createSurfaceView(history);
  const colorScheme = createColorScheme();

  const atEntry = (): boolean => stack.keys().length === 0;
  // Two of the three modes are the glossary.
  const atGlossary = (): boolean =>
    view.mode() === "glossary" || view.mode() === "review";

  // The address the entry surface was left at, which carries its term and its mode
  // and never a reading position. A note's address is written from scratch
  // (`encodeStack`), so the term is gone from the URL the moment a result is
  // opened: nothing but the entry the reader stands on can say what it held.
  //
  // Read off that entry rather than remembered across the surface's whole life: a
  // reader who walks back to a note, or reloads one, arrives on an entry this
  // surface never opened, and memory of another one would send them somewhere they
  // have not been.
  const wayBack = (): string =>
    atEntry() ? history.read() : readEntryAddress();
  let leftAt = wayBack();

  onCleanup(
    onPopState(() => {
      stack.sync();
      view.sync();
      leftAt = wayBack();
    }),
  );

  const openNote = (key: string): void => {
    leftAt = history.read();
    // Onto the entry being left, which the push carries forward, so the reading
    // entry it opens holds the way back as well.
    writeEntryAddress(leftAt);
    stack.open(key);
  };

  // Pushing that address and re-reading both stores from it makes Home a normal
  // history entry the back button can undo, and clears the reading position the
  // address does not hold. The result cursor rides in the entry's state, which a
  // push carries forward (`browser-history.ts`).
  const goHome = (): void => {
    history.push(leftAt);
    stack.sync();
    view.sync();
  };

  return (
    <div class="app">
      <header class="app-header">
        <Show
          when={atEntry()}
          fallback={
            <button type="button" class="app-header__home" onClick={goHome}>
              slipbox
            </button>
          }
        >
          <span class="app-header__title">slipbox</span>
        </Show>
        <Show when={atEntry()}>
          <nav class="app-nav" aria-label="Entry surface">
            <button
              type="button"
              class="app-nav__tab"
              classList={{ "app-nav__tab--active": view.mode() === "search" }}
              aria-current={view.mode() === "search"}
              onClick={() => view.show("search")}
            >
              Notes
            </button>
            <button
              type="button"
              class="app-nav__tab"
              classList={{ "app-nav__tab--active": atGlossary() }}
              aria-current={atGlossary()}
              onClick={() => view.show("glossary")}
            >
              Glossary
            </button>
          </nav>
        </Show>
        <button
          type="button"
          class="app-header__scheme"
          aria-label={`Color scheme: ${SCHEME_LABELS[colorScheme.scheme()]}`}
          onClick={colorScheme.cycle}
        >
          {SCHEME_LABELS[colorScheme.scheme()]}
        </button>
      </header>
      <Show when={atEntry()} fallback={<Spine stack={stack} />}>
        <Show when={atGlossary()} fallback={<EntrySurface onOpen={openNote} />}>
          <GlossaryDictionary
            onOpen={openNote}
            mode={view.mode() === "review" ? "study" : "browse"}
            onMode={(next) => view.show(next === "study" ? "review" : "glossary")}
          />
        </Show>
      </Show>
    </div>
  );
};
