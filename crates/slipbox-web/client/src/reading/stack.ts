import { createSignal, type Accessor } from "solid-js";

import { noteIdentities, type NoteIdentities } from "./note-identity.js";

const NOTE_PARAM = "note";
const STACKED_PARAM = "stacked";

export interface StackHistory {
  readonly read: () => string;
  readonly push: (url: string) => void;
  readonly replace: (url: string) => void;
}

export interface FollowOutcome {
  readonly keys: string[];
  readonly index: number;
  readonly wasOpen: boolean;
}

export interface ReadingStack {
  readonly keys: Accessor<readonly string[]>;
  readonly follow: (index: number, target: string) => number;
  readonly readOn: (index: number, target: string) => number;
  readonly open: (rootKey: string) => void;
  readonly sync: () => void;
}

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

export function decodeStack(url: string): string[] {
  return [...new Set(parseStack(url))];
}

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
  // A key and an id reference to the same note must not create two columns.
  const sameNote = identities.canonical(target);
  const open = keys.findIndex((key) => identities.canonical(key) === sameNote);
  if (open !== -1) {
    return { keys: [...keys], index: open, wasOpen: true };
  }
  const kept = keys.slice(0, index + 1);
  return { keys: [...kept, target], index: kept.length, wasOpen: false };
}

export function reduceReadOn(
  keys: readonly string[],
  index: number,
  target: string,
  identities: NoteIdentities = noteIdentities,
): FollowOutcome {
  return reduceFollow(keys, index - 1, target, identities);
}

export function createReadingStack(
  history: StackHistory,
  identities: NoteIdentities = noteIdentities,
): ReadingStack {
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

  const settle = (outcome: FollowOutcome): number => {
    if (!outcome.wasOpen) {
      commit(outcome.keys);
    }
    return outcome.index;
  };

  return {
    keys,
    follow: (index, target) => settle(reduceFollow(keys(), index, target, identities)),
    readOn: (index, target) => settle(reduceReadOn(keys(), index, target, identities)),
    open: (rootKey) => {
      commit([rootKey]);
    },
    sync: () => {
      setKeys(adopt());
    },
  };
}
