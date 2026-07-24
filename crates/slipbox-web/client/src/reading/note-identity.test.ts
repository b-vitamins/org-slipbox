import { describe, expect, it } from "vitest";

import type { NodeRecord } from "../api/types.js";
import { createNoteIdentities } from "./note-identity.js";

/** A node record carrying only the identity fields the registry reads. */
function node(key: string, id: string | null): NodeRecord {
  return { node_key: key, explicit_id: id } as NodeRecord;
}

describe("createNoteIdentities", () => {
  it("canonicalizes an unknown reference to itself", () => {
    const identities = createNoteIdentities();
    expect(identities.canonical("file:a.org")).toBe("file:a.org");
    expect(identities.canonical("id:unknown")).toBe("id:unknown");
  });

  it("maps a learned note's id reference onto its key", () => {
    const identities = createNoteIdentities();
    identities.learn(node("file:a.org", "abc"));

    expect(identities.canonical("id:abc")).toBe("file:a.org");
    expect(identities.canonical("file:a.org")).toBe("file:a.org");
  });

  it("keeps distinct notes distinct", () => {
    const identities = createNoteIdentities();
    identities.learn(node("file:a.org", "abc"));
    identities.learn(node("file:b.org", "def"));

    expect(identities.canonical("id:abc")).not.toBe(identities.canonical("id:def"));
  });

  it("learns a note with no explicit id by key alone", () => {
    const identities = createNoteIdentities();
    identities.learn(node("heading:a.org::1", null));

    expect(identities.canonical("heading:a.org::1")).toBe("heading:a.org::1");
  });
});
