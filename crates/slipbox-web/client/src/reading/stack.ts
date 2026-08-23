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

/** The persistence seam: read the current URL, push a new one, rewrite this one. */
export interface StackHistory {
  readonly read: () => string;
  readonly push: (url: string) => void;
  /** Rewrite the current entry, for a correction that is not a navigation. */
  readonly replace: (url: string) => void;
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

/** The references an address names, in the order it names them. */
function parseStack(url: string): string[] {
  const query = url.includes("?") ? url.slice(url.indexOf("?") + 1) : "";
  const params = new URLSearchParams(query);
  const root = params.get(NOTE_PARAM);
  if (root === null || root === "") {
    return [];
  }
  const stacked = params.getAll(STACKED_PARAM).filter((key) => key !== "");
  return [root, ...stacked];
}

/**
 * The stack an address opens: a reference it repeats collapses to the first
 * position holding it. Repeats compare by spelling; a decode has resolved nothing.
 */
export function decodeStack(url: string): string[] {
  return [...new Set(parseStack(url))];
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
  // The stack the address opens, and the address corrected to it. Both ways in run
  // through this - the initial read and a `popstate` - so the correction rewrites
  // the entry it arrived on rather than pushing a step Back would have to undo.
  const adopt = (): readonly string[] => {
    const address = history.read();
    const open = decodeStack(address);
    if (open.length !== parseStack(address).length) {
      history.replace(encodeStack(open));
    }
    return open;
  };

  const [keys, setKeys] = createSignal<readonly string[]>(adopt());

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
      setKeys(adopt());
    },
  };
}
