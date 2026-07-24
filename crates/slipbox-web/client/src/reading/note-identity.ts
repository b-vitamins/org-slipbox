/*
 * Canonical identity for note references. A note is addressable both by slipbox
 * key and by `id:<uuid>`, so two references cannot be compared as strings.
 * Identities are learned from every note the surface resolves; an unresolved
 * reference canonicalizes to itself. The map only grows.
 */

import type { NodeRecord } from "../api/types.js";

export interface NoteIdentities {
  readonly learn: (note: NodeRecord) => void;
  /** The note key `reference` names, or the reference itself when unresolved. */
  readonly canonical: (reference: string) => string;
}

export function createNoteIdentities(): NoteIdentities {
  const canonicalByReference = new Map<string, string>();
  return {
    learn: (note) => {
      canonicalByReference.set(note.node_key, note.node_key);
      if (note.explicit_id) {
        canonicalByReference.set(`id:${note.explicit_id}`, note.node_key);
      }
    },
    canonical: (reference) => canonicalByReference.get(reference) ?? reference,
  };
}

/** The surface-wide registry, shared by every reader of a reference. */
export const noteIdentities = createNoteIdentities();
