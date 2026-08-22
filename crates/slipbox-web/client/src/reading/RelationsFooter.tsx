/*
 * The relations footer of a reading column: forward links and backlinks. Both
 * come out of the `NoteContext` the column already fetched, so the footer costs
 * no request of its own.
 */

import { For, Show, type Component } from "solid-js";

import type { NoteContext } from "../api/types.js";
import { GrammarLink } from "../org/GrammarLink.jsx";
import { RenderPreview } from "../org/RenderPreview.jsx";
import {
  backwardRelations,
  forwardRelations,
  type RelationRow,
} from "./relations.js";

const RelationGroup: Component<{ label: string; rows: RelationRow[] }> = (
  props,
) => (
  <Show when={props.rows.length > 0}>
    <div class="relations__group">
      <h2 class="relations__label">{props.label}</h2>
      <ul class="relations__list">
        <For each={props.rows}>
          {(row) => (
            <li class="relations__row">
              <GrammarLink class="relations__link" target={row.target}>
                {row.title}
              </GrammarLink>
              <Show when={row.preview.length > 0}>
                <RenderPreview prose={row.preview} class="relations__preview" />
              </Show>
            </li>
          )}
        </For>
      </ul>
    </div>
  </Show>
);

export const RelationsFooter: Component<{ context: NoteContext }> = (props) => {
  const forward = (): RelationRow[] => forwardRelations(props.context);
  const backward = (): RelationRow[] => backwardRelations(props.context);

  return (
    // The footer carries a rule above it, which would otherwise be drawn under
    // an unlinked note as a line with nothing beneath it.
    <Show when={forward().length > 0 || backward().length > 0}>
      <footer class="relations">
        <RelationGroup label="Links to" rows={forward()} />
        <RelationGroup label="Linked from" rows={backward()} />
      </footer>
    </Show>
  );
};
