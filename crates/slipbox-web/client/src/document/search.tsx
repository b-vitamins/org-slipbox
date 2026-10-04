/*
 * A host-driven search result list for native clients. It deliberately reuses
 * the web entry surface's excerpt reduction and inline renderer, so Org links,
 * emphasis, and TeX never leak through as source syntax.
 */

import { For, Show, createMemo, createSignal } from "solid-js";
import { render } from "solid-js/web";

import type { ContentSegment } from "../api/types.js";
import { excerptRuns } from "../entry/excerpt.js";
import { RenderPreview } from "../org/RenderPreview.jsx";
import "../styles/tokens.css";
import "../org/org.css";
import "./search.css";

export interface OrgSearchResult {
  readonly key: string;
  readonly title: string;
  readonly tags: readonly string[];
  readonly term: boolean;
  readonly excerpt: readonly ContentSegment[];
}

export interface OrgSearchOptions {
  readonly results: readonly OrgSearchResult[];
  readonly theme?: "light" | "dark";
  readonly onOpen: (key: string) => void;
}

export interface OrgSearchHandle {
  readonly update: (options: OrgSearchOptions) => void;
  readonly dispose: () => void;
}

/** Mount the compact search cards used by an embedding native surface. */
export function mountOrgSearch(
  host: Element,
  options: OrgSearchOptions,
): OrgSearchHandle {
  const [mounted, setMounted] = createSignal(options);
  const container = host.ownerDocument.createElement("div");
  container.className = "org-search-host";
  host.append(container);

  const applyTheme = (): void => {
    container.dataset.theme = mounted().theme ?? "light";
  };
  applyTheme();

  const dispose = render(
    () => (
      <ul class="org-search-results" aria-label="Search results">
        <For each={mounted().results}>
          {(result) => {
            const runs = createMemo(() => excerptRuns(result.excerpt));
            return (
              <li class="org-search-result">
                <button
                  type="button"
                  class="org-search-result__button"
                  onClick={() => mounted().onOpen(result.key)}
                >
                  <span class="org-search-result__title">{result.title}</span>
                  <Show when={result.tags.length > 0 || result.term}>
                    <span class="org-search-result__tags">
                      <For each={result.tags}>
                        {(tag) => <span class="org-search-result__tag">{tag}</span>}
                      </For>
                      <Show when={result.term}>
                        <span class="org-search-result__tag">term</span>
                      </Show>
                    </span>
                  </Show>
                  <Show when={runs().length > 0}>
                    <span class="org-search-result__excerpt">
                      <For each={runs()}>
                        {(run) => (
                          <Show
                            when={run.matched}
                            fallback={
                              <RenderPreview
                                prose={run.prose}
                                class="org-search-result__run"
                              />
                            }
                          >
                            <mark class="org-search-result__match">
                              <RenderPreview
                                prose={run.prose}
                                class="org-search-result__run"
                              />
                            </mark>
                          </Show>
                        )}
                      </For>
                    </span>
                  </Show>
                </button>
              </li>
            );
          }}
        </For>
      </ul>
    ),
    container,
  );

  let live = true;
  return {
    update: (next) => {
      if (!live) return;
      setMounted(next);
      applyTheme();
    },
    dispose: () => {
      if (!live) return;
      live = false;
      dispose();
      container.remove();
    },
  };
}
