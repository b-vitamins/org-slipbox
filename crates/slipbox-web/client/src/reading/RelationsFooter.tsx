/*
 * The relations footer of a reading column. The directed inventory - one row per
 * related note, marked with the direction its links run in - comes out of the
 * `NoteContext` the column already fetched, so it costs no request of its own.
 *
 * Beside it stand deferred groups, which answer a question the context does not:
 * nothing is requested for one until a reader opens it, keeping a column's cost
 * at one request for as long as the footer is only read.
 */

import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  type Component,
  type JSX,
} from "solid-js";

import { ApiError, client } from "../api/client.js";
import type { NoteContext } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { GrammarLink } from "../org/GrammarLink.jsx";
import { RenderPreview } from "../org/RenderPreview.jsx";
import {
  boundedRelated,
  rankedTotal,
  relatedGroups,
  relatedRowTotal,
  type RelatedRow,
} from "./related.js";
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
 * Candidates the bridges lens is asked to rank. Its own cut on the answer, and a
 * different one from the head the group shows: the corpus's densest focus note
 * reaches about twenty candidates, so this bounds a pathological note rather
 * than a usual one.
 */
export const RELATED_LENS_LIMIT = 50;

/** Ranked related notes shown before the rest is offered. */
export const RELATED_SHOWN = 8;

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

function describeError(error: unknown, fallback: string): string {
  if (error instanceof ApiError) {
    return error.isNotFound ? "This note is not in the slipbox." : error.message;
  }
  return fallback;
}

/**
 * A group whose content is fetched only once it is opened. The caller owns the
 * open state, since the resource it defers is keyed on that state.
 */
const DeferredGroup: Component<{
  label: string;
  open: boolean;
  onToggle: () => void;
  /** A stated failure, which stands with the group closed. */
  failure?: string;
  children?: JSX.Element;
}> = (props) => (
  <div class="relations__group">
    {/* The label is the control: a heading with a button repeating it beside
        would name the group twice. `aria-expanded` carries the state, so the
        marker the cascade draws is decoration and says nothing of its own. */}
    <h2 class="relations__label">
      <button
        type="button"
        class="relations__toggle"
        aria-expanded={props.open}
        onClick={() => props.onToggle()}
      >
        {props.label}
      </button>
    </h2>
    <Show when={props.failure}>
      {(stated) => (
        <p class="relations__status relations__status--error">{stated()}</p>
      )}
    </Show>
    <Show when={props.open}>{props.children}</Show>
  </div>
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

const RelatedList: Component<{ rows: readonly RelatedRow[] }> = (props) => (
  <ul class="relations__list" role="list">
    <For each={props.rows}>
      {(row) => (
        <li class="relations__row">
          <GrammarLink class="relations__link" target={row.target}>
            {row.title}
          </GrammarLink>
          {/* One connector is what the group's own label already says. */}
          <Show when={row.connectors > 1}>
            <span class="relations__connectors">
              through {row.connectors} notes
            </span>
          </Show>
        </li>
      )}
    </For>
  </ul>
);

/**
 * Notes the bridges lens ranks beside this one, none of which it links to. Two
 * bounds cut the answer and a row count tells them apart from neither, so the
 * head states what it holds back and the lens's own limit is stated separately.
 */
const RelatedNotes: Component<{
  nodeKey: string;
  /** Keys the directed inventory prints, which are not news a second time. */
  listed: ReadonlySet<string>;
}> = (props) => {
  const [open, setOpen] = createSignal(false);
  const [showAll, setShowAll] = createSignal(false);
  const [failure, setFailure] = createSignal<string | undefined>(undefined);

  const related = createReadingResource(
    () => (open() ? props.nodeKey : false),
    (key) => client.explore(key, "bridges", { limit: RELATED_LENS_LIMIT }),
  );

  // A failed read leaves the group closed and says why, so the control that
  // opened it is also the way to ask again.
  createEffect(() => {
    const error = related.error();
    if (error !== undefined) {
      setFailure(describeError(error, "The related notes could not be read."));
      setOpen(false);
    }
  });

  const groups = createMemo(() => {
    const answer = related.ready();
    return answer ? relatedGroups(answer, props.listed) : [];
  });
  const total = (): number => relatedRowTotal(groups());
  const head = createMemo(() =>
    showAll() ? groups() : boundedRelated(groups(), RELATED_SHOWN),
  );
  const held = (): number => total() - relatedRowTotal(head());

  return (
    <DeferredGroup
      label="Related notes"
      open={open()}
      failure={failure()}
      onToggle={() => {
        setFailure(undefined);
        setOpen((was) => !was);
      }}
    >
      <Show
        when={related.ready()}
        fallback={<p class="relations__status">Ranking related notes…</p>}
      >
        {(answer) => (
          <Show
            when={total() > 0}
            fallback={
              <p class="relations__status">
                No note stands beside this one unlinked.
              </p>
            }
          >
            <For each={head()}>
              {(group) => (
                <div class="relations__connected">
                  <h3 class="relations__connector">via {group.connector}</h3>
                  <RelatedList rows={group.rows} />
                </div>
              )}
            </For>
            <Show when={held() > 0}>
              <button
                type="button"
                class="relations__more"
                onClick={() => setShowAll(true)}
              >
                Show {held()} more
              </button>
            </Show>
            {/* The head's cut is the control above; this is the other bound, and
                the lens reports no total to measure it against. */}
            <Show when={rankedTotal(answer()) >= RELATED_LENS_LIMIT}>
              <p class="relations__shortfall">
                The lens ranks at most {RELATED_LENS_LIMIT} notes, so the
                slipbox may hold more.
              </p>
            </Show>
          </Show>
        )}
      </Show>
    </DeferredGroup>
  );
};

export const RelationsFooter: Component<{ context: NoteContext }> = (props) => {
  const rows = (): RelationRow[] => relationRows(props.context);
  const listed = (): ReadonlySet<string> =>
    new Set(rows().map((row) => row.key));

  return (
    // The footer carries a rule above it, which would otherwise be drawn under
    // an unlinked note as a line with nothing beneath it. A note with no link
    // has no bridge candidate either: the lens walks link topology, so the
    // deferred group below would answer with nothing for one.
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
        <RelatedNotes
          nodeKey={props.context.note.node_key}
          listed={listed()}
        />
      </footer>
    </Show>
  );
};
