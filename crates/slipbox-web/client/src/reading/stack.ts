/*
 * The reading stack: the notes on screen, mirrored to the URL as
 * `?note=<root>&stacked=<a>&stacked=<b>`. Following a link from column `n`
 * prunes everything to its right and appends the target, unless it is already on
 * screen. That comparison is by note identity: a note has two spellings.
 */

import { createSignal, type Accessor } from "solid-js";

import { noteIdentities, type NoteIdentities } from "./note-identity.js";

const NOTE_PARAM = "note";
const STACKED_PARAM = "stacked";

/** The persistence seam: read the current URL and push a new one. */
export interface StackHistory {
  readonly read: () => string;
  readonly push: (url: string) => void;
}

export interface FollowOutcome {
  readonly keys: string[];
  /** The index now holding the target. */
  readonly index: number;
  /** True when the target was already open, so the stack is unchanged. */
  readonly wasOpen: boolean;
}

export interface ReadingStack {
  /** The live list of note keys, `[root, ...stacked]`. */
  readonly keys: Accessor<readonly string[]>;
  /**
   * Open `target` from column `index`, pruning columns to its right, or leave the
   * stack as it stands when the target is already on screen. Returns the index
   * now holding the target, for the caller to reveal.
   */
  readonly follow: (index: number, target: string) => number;
  /** Replace the whole stack. */
  readonly open: (rootKey: string) => void;
  /** Re-read the stack from the current URL (for `popstate`). */
  readonly sync: () => void;
}

export function decodeStack(url: string): string[] {
  const query = url.includes("?") ? url.slice(url.indexOf("?") + 1) : "";
  const params = new URLSearchParams(query);
  const root = params.get(NOTE_PARAM);
  if (root === null || root === "") {
    return [];
  }
  const stacked = params.getAll(STACKED_PARAM).filter((key) => key !== "");
  return [root, ...stacked];
}

/** Returns the empty string, not `"?"`, for an empty stack. */
export function encodeStack(keys: readonly string[]): string {
  const [root, ...stacked] = keys;
  if (root === undefined) {
    return "";
  }
  const params = new URLSearchParams();
  params.set(NOTE_PARAM, root);
  for (const key of stacked) {
    params.append(STACKED_PARAM, key);
  }
  return `?${params.toString()}`;
}

export function reduceFollow(
  keys: readonly string[],
  index: number,
  target: string,
  identities: NoteIdentities = noteIdentities,
): FollowOutcome {
  // Compared by the note named, not by the reference naming it: the same note
  // reached by key and by `id:` is one column.
  const sameNote = identities.canonical(target);
  const open = keys.findIndex((key) => identities.canonical(key) === sameNote);
  if (open !== -1) {
    return { keys: [...keys], index: open, wasOpen: true };
  }
  const kept = keys.slice(0, index + 1);
  return { keys: [...kept, target], index: kept.length, wasOpen: false };
}

export function createReadingStack(
  history: StackHistory,
  identities: NoteIdentities = noteIdentities,
): ReadingStack {
  const [keys, setKeys] = createSignal<readonly string[]>(
    decodeStack(history.read()),
  );

  const commit = (next: readonly string[]): void => {
    setKeys(next);
    history.push(encodeStack(next));
  };

  return {
    keys,
    follow: (index, target) => {
      const next = reduceFollow(keys(), index, target, identities);
      // Revealing an open note changes neither the stack nor the URL, so it must
      // push no history entry.
      if (!next.wasOpen) {
        commit(next.keys);
      }
      return next.index;
    },
    open: (rootKey) => {
      commit([rootKey]);
    },
    sync: () => {
      setKeys(decodeStack(history.read()));
    },
  };
}
