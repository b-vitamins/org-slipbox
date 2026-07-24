/*
 * The reading-surface shell.
 *
 * The shell owns the reading stack — the ordered notes on screen, mirrored to
 * the URL — and chooses what fills the frame: the reading spine when the URL
 * names at least one note, and an identity card (the served root and index
 * counts) as the resting empty state before a note is opened. Search and the
 * glossary mount into this same frame in later milestones.
 */

import { Show, onCleanup, type Component } from "solid-js";

import { client } from "./api/client.js";
import { ApiError } from "./api/client.js";
import { createReadingResource } from "./data/create-reading-resource.js";
import { useDocumentTitle } from "./dom/document-title.js";
import { browserHistory, onPopState } from "./reading/browser-history.js";
import { Spine } from "./reading/Spine.jsx";
import { createReadingStack } from "./reading/stack.js";
import "./styles/tokens.css";
import "./App.css";

function describeError(error: unknown): string {
  if (error instanceof ApiError) {
    return `${error.kind}: ${error.message}`;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "the reading surface is unreachable";
}

const IdentityCard: Component = () => {
  const status = createReadingResource(
    () => true,
    () => client.status(),
  );

  // The resting empty state owns the browser tab: it reflects an unreachable
  // surface so multiple open tabs stay distinguishable and never leave the
  // static build-time title in place. The served root is a filesystem path, so
  // it is deliberately not used here; note and search labels arrive from the
  // views of later milestones.
  useDocumentTitle(() => (status.error() ? "Unavailable" : undefined));

  return (
    <main class="app-main" aria-live="polite" aria-busy={status.loading()}>
      <Show
        when={status.error()}
        fallback={
          <Show
            when={status.ready()}
            fallback={<p class="app-note">Reading the slipbox…</p>}
          >
            {(info) => (
              <section class="status-card">
                <h1 class="status-card__root">{info().root}</h1>
                <dl class="status-card__facts">
                  <div class="status-card__fact">
                    <dt>Notes</dt>
                    <dd>{info().nodes_indexed}</dd>
                  </div>
                  <div class="status-card__fact">
                    <dt>Files</dt>
                    <dd>{info().files_indexed}</dd>
                  </div>
                  <div class="status-card__fact">
                    <dt>Links</dt>
                    <dd>{info().links_indexed}</dd>
                  </div>
                </dl>
                <p class="status-card__version">slipbox {info().version}</p>
              </section>
            )}
          </Show>
        }
      >
        {(error) => (
          <p class="app-note app-note--error">{describeError(error())}</p>
        )}
      </Show>
    </main>
  );
};

export const App: Component = () => {
  const stack = createReadingStack(browserHistory());
  onCleanup(onPopState(() => stack.sync()));

  return (
    <div class="app">
      <header class="app-header">
        <span class="app-header__title">slipbox</span>
      </header>
      <Show when={stack.keys().length > 0} fallback={<IdentityCard />}>
        <Spine stack={stack} />
      </Show>
    </div>
  );
};
