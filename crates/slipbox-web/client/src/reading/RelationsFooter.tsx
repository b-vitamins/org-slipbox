/*
 * The relations footer of a reading column: forward links, backlinks, and the
 * opt-in bounded neighborhood. The link rows come out of the `NoteContext` the
 * column already fetched, so only the neighborhood costs a further request.
 */

import { For, Show, type Component } from "solid-js";

import type { NoteContext } from "../api/types.js";
import { GrammarLink } from "../org/GrammarLink.jsx";
import { RenderPreview } from "../org/RenderPreview.jsx";
import { NeighborhoodRings } from "./NeighborhoodRings.jsx";
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

  // Keys the link groups already list, kept out of the rings.
  const shown = (): Set<string> =>
    new Set([...forward(), ...backward()].map((row) => row.key));

  return (
    <footer class="relations">
      <RelationGroup label="Links to" rows={forward()} />
      <RelationGroup label="Linked from" rows={backward()} />
      <NeighborhoodRings
        nodeKey={props.context.note.node_key}
        shown={shown()}
      />
    </footer>
  );
};
