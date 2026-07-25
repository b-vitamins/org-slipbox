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
import { browserHistory, onPopState } from "./reading/browser-history.js";
import { Spine } from "./reading/Spine.jsx";
import { createReadingStack } from "./reading/stack.js";
import "./styles/tokens.css";
import "./App.css";

export const App: Component = () => {
  const history = browserHistory();
  const stack = createReadingStack(history);
  const view = createSurfaceView(history);
  const colorScheme = createColorScheme();
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
