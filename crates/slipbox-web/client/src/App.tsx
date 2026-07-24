/*
 * The reading-surface shell.
 *
 * This milestone scaffolds the client: it stands up the toolchain, the typed
 * API layer, and the focus-aware data layer, then proves the whole stack end
 * to end by reading `/api/status` from a real read-only daemon and rendering
 * the served identity. The reading column, the stacked spine, navigation, and
 * search arrive in later milestones; this is the frame they mount into.
 */

import { Show, type Component } from "solid-js";

import { client } from "./api/client.js";
import { ApiError } from "./api/client.js";
import { createReadingResource } from "./data/create-reading-resource.js";
import { useDocumentTitle } from "./dom/document-title.js";
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

export const App: Component = () => {
  const status = createReadingResource(
    () => true,
    () => client.status(),
  );

  // Drive the browser tab from state so multiple open tabs stay distinguishable
  // and never leave the static build-time title in place. The served root is a
  // filesystem path, so it is deliberately not used here; specific labels (note
  // titles, search queries) arrive from the views of later milestones.
  useDocumentTitle(() => (status.error() ? "Unavailable" : undefined));

  return (
    <div class="app">
      <header class="app-header">
        <span class="app-header__title">slipbox</span>
      </header>
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
    </div>
  );
};
