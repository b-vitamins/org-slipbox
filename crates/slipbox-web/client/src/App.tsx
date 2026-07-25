/*
 * The reading-surface shell.
 *
 * The shell owns the reading stack — the ordered notes on screen, mirrored to
 * the URL — and chooses what fills the frame: the reading spine when the URL
 * names at least one note, and otherwise one of two entry surfaces. Which entry
 * surface (the search-first note entry, or the glossary dictionary) is itself
 * URL state, toggled from the header. Opening a note or a term from either
 * surface seeds the stack's root, which swaps the frame to the spine; the back
 * button walks all the way out to the entry again. Both the stack and the
 * surface mode re-read from the URL on `popstate`, so history stays authoritative.
 *
 * The header wordmark doubles as the in-app exit: while a note is open it is a
 * Home button back to the entry, so a shared, bookmarked, or missing-note URL is
 * never a dead end reachable only through browser chrome.
 */

import { Show, onCleanup, type Component } from "solid-js";

import { EntrySurface } from "./entry/EntrySurface.jsx";
import { GlossaryDictionary } from "./entry/GlossaryDictionary.jsx";
import { createSurfaceView } from "./entry/surface-view.js";
import { browserHistory, onPopState } from "./reading/browser-history.js";
import { Spine } from "./reading/Spine.jsx";
import { createReadingStack } from "./reading/stack.js";
import "./styles/tokens.css";
import "./App.css";

export const App: Component = () => {
  const history = browserHistory();
  const stack = createReadingStack(history);
  const view = createSurfaceView(history);
  onCleanup(
    onPopState(() => {
      stack.sync();
      view.sync();
    }),
  );

  const atEntry = (): boolean => stack.keys().length === 0;
  // Two of the three modes are the glossary.
  const atGlossary = (): boolean =>
    view.mode() === "glossary" || view.mode() === "review";

  // Pushing the empty URL and re-reading both stores from it makes Home a normal
  // history entry the back button can undo.
  const goHome = (): void => {
    history.push("");
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
      </header>
      <Show when={atEntry()} fallback={<Spine stack={stack} />}>
        <Show when={atGlossary()} fallback={<EntrySurface onOpen={stack.open} />}>
          <GlossaryDictionary
            onOpen={stack.open}
            mode={view.mode() === "review" ? "study" : "browse"}
            onMode={(next) => view.show(next === "study" ? "review" : "glossary")}
          />
        </Show>
      </Show>
    </div>
  );
};
