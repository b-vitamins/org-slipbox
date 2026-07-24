import { afterEach, describe, expect, it, vi } from "vitest";

import { noteIdentities } from "./note-identity.js";
import {
  PREVIEW_MAX_LINES,
  WHOLE_NOTE_MAX_LINES,
  fetchNoteContext,
} from "./fetch-note.js";

function json(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

function contextFor(key: string, explicitId: string | null = null): unknown {
  return {
    note: { node_key: key, explicit_id: explicitId, title: "Alpha" },
    source: { content: "#+title: Alpha\n" },
    node_start_line: 1,
    node_line_count: 1,
    backlinks: [],
    forward_links: [],
  };
}

/** A fetch double recording every URL, answering each by route prefix. */
function routedFetch(routes: Record<string, unknown>): {
  fetch: typeof fetch;
  calls: string[];
} {
  const calls: string[] = [];
  const impl = vi.fn((input: RequestInfo | URL) => {
    const url = String(input);
    calls.push(url);
    const route = Object.keys(routes).find((prefix) => url.startsWith(prefix));
    if (!route) {
      return Promise.resolve(
        new Response(JSON.stringify({ error: { kind: "not-found", message: url } }), {
          status: 404,
          headers: { "content-type": "application/json" },
        }),
      );
    }
    return Promise.resolve(json(routes[route]));
  });
  return { fetch: impl as unknown as typeof fetch, calls };
}

describe("fetchNoteContext", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("fetches a raw key's context in one request", async () => {
    const { fetch, calls } = routedFetch({
      "/api/note/context": contextFor("file:notes/alpha.org"),
    });
    vi.stubGlobal("fetch", fetch);

    const context = await fetchNoteContext("file:notes/alpha.org", WHOLE_NOTE_MAX_LINES);

    expect(calls).toEqual([
      "/api/note/context?key=file%3Anotes%2Falpha.org&max_lines=1000",
    ]);
    expect(context.note.node_key).toBe("file:notes/alpha.org");
  });

  it("resolves an id reference to its node before reading the note", async () => {
    const { fetch, calls } = routedFetch({
      "/api/node": {
        node_key: "file:notes/alpha.org",
        explicit_id: "alpha-id",
        title: "Alpha",
      },
      "/api/note/context": contextFor("file:notes/alpha.org", "alpha-id"),
    });
    vi.stubGlobal("fetch", fetch);

    const context = await fetchNoteContext("id:alpha-id", WHOLE_NOTE_MAX_LINES);

    // The id resolves to a node, and that node's key is what the context is read
    // by: an id must never reach the context route.
    expect(calls).toEqual([
      "/api/node?id=alpha-id",
      "/api/note/context?key=file%3Anotes%2Falpha.org&max_lines=1000",
    ]);
    expect(context.note.node_key).toBe("file:notes/alpha.org");
  });

  it("caps a preview read at the shorter line budget", async () => {
    const { fetch, calls } = routedFetch({
      "/api/note/context": contextFor("file:notes/alpha.org"),
    });
    vi.stubGlobal("fetch", fetch);

    await fetchNoteContext("file:notes/alpha.org", PREVIEW_MAX_LINES);

    expect(calls[0]).toContain("max_lines=40");
    expect(PREVIEW_MAX_LINES).toBeLessThan(WHOLE_NOTE_MAX_LINES);
  });

  it("records the resolved note's identity so both its spellings compare equal", async () => {
    const { fetch } = routedFetch({
      "/api/note/context": contextFor("file:notes/learned.org", "learned-id"),
    });
    vi.stubGlobal("fetch", fetch);

    expect(noteIdentities.canonical("id:learned-id")).toBe("id:learned-id");

    await fetchNoteContext("file:notes/learned.org", WHOLE_NOTE_MAX_LINES);

    expect(noteIdentities.canonical("id:learned-id")).toBe("file:notes/learned.org");
  });

  it("propagates a failed lookup rather than yielding a blank context", async () => {
    const { fetch } = routedFetch({});
    vi.stubGlobal("fetch", fetch);

    await expect(
      fetchNoteContext("file:notes/missing.org", WHOLE_NOTE_MAX_LINES),
    ).rejects.toMatchObject({ name: "ApiError", status: 404 });
  });
});
