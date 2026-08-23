// Immediate relations come from NoteContext; exploration groups fetch on demand.

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
import type { NoteContext, NotePlace, NotePlaceNeighbor } from "../api/types.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { GrammarLink, isBrowserGesture } from "../org/GrammarLink.jsx";
import { RenderInline } from "../org/RenderInline.jsx";
import { RenderPreview } from "../org/RenderPreview.jsx";
import { mentionRows, type MentionRow } from "./mentions.js";
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
import type { FilingMove } from "./spine-navigation.js";

const DIRECTIONS: Record<
  RelationDirection,
  { readonly glyph: string; readonly label: string }
> = {
  out: { glyph: "→", label: "Links to" },
  in: { glyph: "←", label: "Linked from" },
  both: { glyph: "↔", label: "Links to and from" },
};

export const RELATED_LENS_LIMIT = 50;
export const RELATED_SHOWN = 8;

// The scan limit counts occurrences; the display limit counts distinct notes.
export const MENTIONS_SCAN_LIMIT = 200;
export const MENTIONS_SHOWN = 8;

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

const DeferredGroup: Component<{
  label: string;
  open: boolean;
  onToggle: () => void;
  failure?: string;
  children?: JSX.Element;
}> = (props) => (
  <div class="relations__group">
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

const RelatedNotes: Component<{
  nodeKey: string;
  listed: ReadonlySet<string>;
}> = (props) => {
  const [open, setOpen] = createSignal(false);
  const [showAll, setShowAll] = createSignal(false);
  const [failure, setFailure] = createSignal<string | undefined>(undefined);

  const related = createReadingResource(
    () => (open() ? props.nodeKey : false),
    (key) => client.explore(key, "bridges", { limit: RELATED_LENS_LIMIT }),
  );

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

const MentionsList: Component<{ rows: readonly MentionRow[] }> = (props) => (
  <ul class="relations__list" role="list">
    <For each={props.rows}>
      {(row) => (
        <li class="relations__row">
          <GrammarLink class="relations__link" target={row.target}>
            {row.title}
          </GrammarLink>
          <span class="relations__preview">
            <For each={row.preview}>
              {(run) => (
                <Show
                  when={run.matched}
                  fallback={<RenderInline nodes={run.prose} />}
                >
                  <mark class="relations__match">
                    <RenderInline nodes={run.prose} />
                  </mark>
                </Show>
              )}
            </For>
          </span>
        </li>
      )}
    </For>
  </ul>
);

const UnlinkedMentions: Component<{
  nodeKey: string;
  listed: ReadonlySet<string>;
}> = (props) => {
  const [open, setOpen] = createSignal(false);
  const [showAll, setShowAll] = createSignal(false);
  const [failure, setFailure] = createSignal<string | undefined>(undefined);

  const mentions = createReadingResource(
    () => (open() ? props.nodeKey : false),
    (key) => client.unlinkedReferences(key, { limit: MENTIONS_SCAN_LIMIT }),
  );

  createEffect(() => {
    const error = mentions.error();
    if (error !== undefined) {
      setFailure(describeError(error, "The mentions could not be read."));
      setOpen(false);
    }
  });

  const rows = createMemo(() => {
    const answer = mentions.ready();
    return answer ? mentionRows(answer, props.listed) : [];
  });
  const head = createMemo(() =>
    showAll() ? rows() : rows().slice(0, MENTIONS_SHOWN),
  );
  const held = (): number => rows().length - head().length;

  return (
    <DeferredGroup
      label="Unlinked mentions"
      open={open()}
      failure={failure()}
      onToggle={() => {
        setFailure(undefined);
        setOpen((was) => !was);
      }}
    >
      <Show
        when={mentions.ready()}
        fallback={<p class="relations__status">Scanning for mentions…</p>}
      >
        {(answer) => (
          <Show
            when={rows().length > 0}
            fallback={
              <p class="relations__status">
                No note names this one without linking to it.
              </p>
            }
          >
            <MentionsList rows={head()} />
            <Show when={held() > 0}>
              <button
                type="button"
                class="relations__more"
                onClick={() => setShowAll(true)}
              >
                Show {held()} more
              </button>
            </Show>
            <Show
              when={answer().unlinked_references.length >= MENTIONS_SCAN_LIMIT}
            >
              <p class="relations__shortfall">
                The scan stops at {MENTIONS_SCAN_LIMIT} mentions, so the slipbox
                may hold more.
              </p>
            </Show>
          </Show>
        )}
      </Show>
    </DeferredGroup>
  );
};

interface ReadOnSide {
  readonly side: string;
  readonly neighbor: NotePlaceNeighbor;
}

function readOnSides(place: NotePlace | undefined): ReadOnSide[] {
  const earlier = place?.earlier;
  const later = place?.later;
  return [
    ...(earlier ? [{ side: "Filed before this", neighbor: earlier }] : []),
    ...(later ? [{ side: "Filed after this", neighbor: later }] : []),
  ];
}

const ReadOnLink: Component<{ move: ReadOnSide; readOn: FilingMove }> = (props) => (
  <a
    class="read-on__link"
    href={props.readOn.address(props.move.neighbor.node_key)}
    onClick={(event) => {
      if (isBrowserGesture(event)) {
        return;
      }
      event.preventDefault();
      props.readOn.open(props.move.neighbor.node_key);
    }}
  >
    <span class="read-on__side">{props.move.side}</span>
    <span class="read-on__title">{props.move.neighbor.title}</span>
  </a>
);

export const RelationsFooter: Component<{
  context: NoteContext;
  readOn: FilingMove;
}> = (props) => {
  const rows = (): RelationRow[] => relationRows(props.context);
  const listed = (): ReadonlySet<string> =>
    new Set(rows().map((row) => row.key));
  const sides = (): ReadOnSide[] => readOnSides(props.context.place);

  return (
    <>
      <Show when={sides().length > 0}>
        <div class="read-on">
          <For each={sides()}>
            {(move) => <ReadOnLink move={move} readOn={props.readOn} />}
          </For>
        </div>
      </Show>
      <footer class="relations">
        <Show when={rows().length > 0}>
          <div class="relations__group">
            <h2 class="relations__label">Links</h2>
            <RelationsList rows={rows()} />
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
          <RelatedNotes nodeKey={props.context.note.node_key} listed={listed()} />
        </Show>
        <UnlinkedMentions
          nodeKey={props.context.note.node_key}
          listed={listed()}
        />
      </footer>
    </>
  );
};
