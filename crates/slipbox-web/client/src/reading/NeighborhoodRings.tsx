/*
 * The bounded-neighborhood view: concentric distance rings, ranked and bounded
 * by `distanceRings`/`nearestOf`, with a title filter. Opening the disclosure
 * flips the resource's source from `false` (deferred) to the origin key, so
 * nothing is fetched until then.
 */

import { For, Show, createMemo, createSignal, type Component } from "solid-js";

import { ApiError, client } from "../api/client.js";
import { createReadingResource } from "../data/create-reading-resource.js";
import { GrammarLink } from "../org/GrammarLink.jsx";
import {
  distanceRings,
  nearestOf,
  ringPopulation,
  type DistanceRing,
} from "./neighborhood.js";

/** Hops the walk asks the server for. */
const HOPS = 2;

/** Members of a ring shown before the rest is offered; cuts on the ranking. */
const NEAREST_SHOWN = 8;

function describeError(error: unknown): string {
  if (error instanceof ApiError) {
    return error.isNotFound
      ? "This note is not in the slipbox."
      : error.message;
  }
  return "The neighborhood could not be read.";
}

function ringLabel(distance: number): string {
  return distance === 1 ? "1 hop away" : `${distance} hops away`;
}

/** Case-insensitive substring match on the title; emptied rings are dropped. */
function filtered(rings: DistanceRing[], filter: string): DistanceRing[] {
  const needle = filter.trim().toLowerCase();
  if (needle === "") {
    return rings;
  }
  return rings
    .map((ring) => ({
      distance: ring.distance,
      members: ring.members.filter((member) =>
        member.title.toLowerCase().includes(needle),
      ),
    }))
    .filter((ring) => ring.members.length > 0);
}

export const NeighborhoodRings: Component<{
  nodeKey: string;
  /** Keys already shown as immediate relations, kept out of the rings. */
  shown?: ReadonlySet<string>;
}> = (props) => {
  const [open, setOpen] = createSignal(false);
  const [filter, setFilter] = createSignal("");
  const [showAll, setShowAll] = createSignal(false);

  const neighborhood = createReadingResource(
    () => (open() ? props.nodeKey : false),
    (key) => client.neighborhood(key, { hops: HOPS }),
  );

  const rings = createMemo<DistanceRing[]>(() => {
    const value = neighborhood.ready();
    return value ? distanceRings(value, props.shown ?? new Set()) : [];
  });

  // Filter before bound: a match ranked below the cut must still be findable.
  const matched = createMemo<DistanceRing[]>(() => filtered(rings(), filter()));
  const bounded = createMemo(() =>
    showAll() ? { rings: matched(), hidden: 0 } : nearestOf(matched(), NEAREST_SHOWN),
  );

  const worthFiltering = (): boolean => ringPopulation(rings()) > NEAREST_SHOWN;

  return (
    <section class="neighborhood">
      <button
        type="button"
        class="neighborhood__toggle"
        aria-expanded={open()}
        onClick={() => setOpen((was) => !was)}
      >
        {open() ? "Hide neighborhood" : "Explore neighborhood"}
      </button>

      <Show when={open()}>
        <Show
          when={!neighborhood.error()}
          fallback={
            <p class="reading-note__status reading-note__status--error">
              {describeError(neighborhood.error())}
            </p>
          }
        >
          <Show
            when={neighborhood.ready()}
            fallback={
              <p class="reading-note__status">Walking the neighborhood…</p>
            }
          >
            {(loaded) => (
              <Show
                when={rings().length > 0}
                fallback={
                  <p class="reading-note__status">
                    This note has no neighbors.
                  </p>
                }
              >
                <Show when={worthFiltering()}>
                  <input
                    type="search"
                    class="neighborhood__filter"
                    placeholder="Filter these notes"
                    autocomplete="off"
                    aria-label="Filter the neighborhood"
                    value={filter()}
                    onInput={(event) => setFilter(event.currentTarget.value)}
                  />
                </Show>

                <For each={bounded().rings}>
                  {(ring) => (
                    <div class="neighborhood__ring">
                      <h3 class="neighborhood__distance">
                        {ringLabel(ring.distance)}
                      </h3>
                      <ul class="neighborhood__members">
                        <For each={ring.members}>
                          {(member) => (
                            <li class="neighborhood__member">
                              <GrammarLink
                                class="neighborhood__link"
                                target={member.target}
                              >
                                {member.title}
                              </GrammarLink>
                              <Show when={member.via.length > 0}>
                                <span class="neighborhood__via">
                                  via {member.via.join(", ")}
                                </span>
                              </Show>
                            </li>
                          )}
                        </For>
                      </ul>
                    </div>
                  )}
                </For>

                <Show when={matched().length === 0}>
                  <p class="reading-note__status">
                    No nearby notes match that filter.
                  </p>
                </Show>

                <Show when={bounded().hidden > 0}>
                  <button
                    type="button"
                    class="neighborhood__more"
                    onClick={() => setShowAll(true)}
                  >
                    Show {bounded().hidden} more
                  </button>
                </Show>

                <Show when={loaded().truncated}>
                  <p class="neighborhood__truncated">
                    Showing the nearest notes; the neighborhood is larger.
                  </p>
                </Show>
              </Show>
            )}
          </Show>
        </Show>
      </Show>
    </section>
  );
};
