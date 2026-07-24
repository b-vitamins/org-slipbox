/*
 * The reading-surface shell.
 *
 * The shell owns the reading stack — the ordered notes on screen, mirrored to
 * the URL — and chooses what fills the frame: the reading spine when the URL
 * names at least one note, and the search-first entry surface (the served
 * identity and a search field) as the resting empty state before a note is
 * opened. Opening a note from search seeds the stack's root, which swaps the
 * frame to the spine. The glossary mounts into this same frame in a later
 * milestone.
 */

import { Show, onCleanup, type Component } from "solid-js";

import { EntrySurface } from "./entry/EntrySurface.jsx";
import { browserHistory, onPopState } from "./reading/browser-history.js";
import { Spine } from "./reading/Spine.jsx";
import { createReadingStack } from "./reading/stack.js";
import "./styles/tokens.css";
import "./App.css";

export const App: Component = () => {
  const stack = createReadingStack(browserHistory());
  onCleanup(onPopState(() => stack.sync()));

  return (
    <div class="app">
      <header class="app-header">
        <span class="app-header__title">slipbox</span>
      </header>
      <Show
        when={stack.keys().length > 0}
        fallback={<EntrySurface onOpen={stack.open} />}
      >
        <Spine stack={stack} />
      </Show>
    </div>
  );
};
