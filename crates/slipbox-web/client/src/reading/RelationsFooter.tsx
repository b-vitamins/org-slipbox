/*
 * The relations footer of a reading column: one row per related note, marked
 * with the direction its links run in. The rows come out of the `NoteContext`
 * the column already fetched, so the footer costs no request of its own.
 */

import { For, Show, type Component } from "solid-js";

import type { NoteContext } from "../api/types.js";
import { GrammarLink } from "../org/GrammarLink.jsx";
import { RenderPreview } from "../org/RenderPreview.jsx";
import {
  relationRows,
  shownInDirection,
  type RelationDirection,
  type RelationRow,
} from "./relations.js";

/**
 * The mark a row carries for its direction, and what that mark is read as. The
 * glyph alone is not a label, so it is spoken through the `img` role's own
 * accessible name rather than left for a screen reader to pronounce.
 */
const DIRECTIONS: Record<
  RelationDirection,
  { readonly glyph: string; readonly label: string }
> = {
  out: { glyph: "→", label: "Links to" },
  in: { glyph: "←", label: "Linked from" },
  both: { glyph: "↔", label: "Links to and from" },
};

/**
 * What a bounded group holds back. The total is the payload's own count, which a
 * row count cannot stand in for: a cut set and a whole one look alike. The
 * subject names which count it is, since the group's rows hold both directions.
 *
 * A payload that carries no total, as a daemon older than the field answers,
 * leaves the size of the set unknown, and nothing is claimed of an unknown.
 */
export const RelationShortfall: Component<{
  shown: number;
  total?: number;
  subject: string;
}> = (props) => (
  <Show when={props.total !== undefined && props.total > props.shown}>
    <p class="relations__shortfall">
      Showing {props.shown} of {props.total} {props.subject}.
    </p>
  </Show>
);

const RelationsList: Component<{ rows: RelationRow[] }> = (props) => (
  <ul class="relations__list" role="list">
    <For each={props.rows}>
      {(row) => (
        <li class="relations__row">
          <span
            class="relations__direction"
            role="img"
            aria-label={DIRECTIONS[row.direction].label}
          >
            {DIRECTIONS[row.direction].glyph}
          </span>
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
);

export const RelationsFooter: Component<{ context: NoteContext }> = (props) => {
  const rows = (): RelationRow[] => relationRows(props.context);

  return (
    // The footer carries a rule above it, which would otherwise be drawn under
    // an unlinked note as a line with nothing beneath it.
    <Show when={rows().length > 0}>
      <footer class="relations">
        <div class="relations__group">
          <h2 class="relations__label">Links</h2>
          <RelationsList rows={rows()} />
          {/* The request bounds each direction of its own, so a cut is stated
              per direction even though the rows are one listing. */}
          <RelationShortfall
            shown={shownInDirection(rows(), "out")}
            total={props.context.forward_link_note_total}
            subject="notes linked to"
          />
          <RelationShortfall
            shown={shownInDirection(rows(), "in")}
            total={props.context.backlink_note_total}
            subject="notes linking here"
          />
        </div>
      </footer>
    </Show>
  );
};
