/*
 * `createReadingResource` wraps Solid's `createResource` with refetch-on-focus
 * and a flat `loading`/`error`/`ready` view of the request lifecycle.
 *
 * A resolved value is stored alongside the key it answers, because Solid retains
 * the previous value while the next request is in flight; comparing keys is what
 * keeps `ready` from reporting one key's value under another's.
 */

import { createResource, onCleanup, type Accessor } from "solid-js";

import { onRefocus } from "./refetch-on-focus.js";

export interface ReadingResource<T> {
  /**
   * The resolved value for the current key, or `undefined` while loading,
   * unresolved, deferred, or errored. Unlike Solid's `resource.latest`, this
   * never throws on error.
   */
  readonly ready: Accessor<T | undefined>;
  readonly loading: Accessor<boolean>;
  /** The failure of the request in flight for the current key, if it failed. */
  readonly error: Accessor<unknown>;
  /** Force a refetch (also invoked automatically on refocus). */
  readonly refetch: () => void;
}

/** A resolved value together with the key it answers. */
interface Keyed<S, T> {
  readonly key: S;
  readonly value: T;
}

/**
 * Create a focus-aware reading resource.
 *
 * @param source keyed source; a falsy value defers the fetch (Solid convention)
 * @param fetcher maps a truthy source value to a promise of the result
 */
export function createReadingResource<S, T>(
  source: Accessor<S | false | null | undefined>,
  fetcher: (source: S) => Promise<T>,
): ReadingResource<T> {
  const [resource, { refetch }] = createResource<Keyed<S, T>, S>(
    source,
    async (key) => ({ key, value: await fetcher(key) }),
  );

  const runRefetch = () => {
    void refetch();
  };

  const unregister = onRefocus(runRefetch);
  onCleanup(unregister);

  return {
    // Solid's `latest` throws on `errored` and restarts loading on
    // `unresolved`/`pending`, so it is read only in the two safe states.
    ready: () => {
      const state = resource.state;
      if (state !== "ready" && state !== "refreshing") {
        return undefined;
      }
      const held = resource.latest;
      return held !== undefined && Object.is(held.key, source())
        ? held.value
        : undefined;
    },
    loading: () => resource.loading,
    // `errored` means the newest request failed, so a failure a later request
    // supersedes is not reported.
    error: () => (resource.state === "errored" ? resource.error : undefined),
    refetch: runRefetch,
  };
}
