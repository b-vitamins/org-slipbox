import { describe, expect, it, vi } from "vitest";

import { ApiError, ReadingClient } from "./client.js";
import type {
  ExploreResult,
  NodeRecord,
  UnlinkedReferencesResult,
} from "./types.js";

/** A node record with only the fields a test names given a value. */
function node(key: string, title: string): NodeRecord {
  return {
    node_key: key,
    explicit_id: null,
    file_path: "notes/n.org",
    title,
    outline_path: title,
    aliases: [],
    tags: [],
    refs: [],
    todo_keyword: null,
    scheduled_for: null,
    deadline_for: null,
    closed_at: null,
    glossary: false,
    glossary_status: null,
    sr_due: null,
    sr_ease: null,
    sr_interval: null,
    sr_reps: null,
    sr_last: null,
    level: 0,
    line: 1,
    kind: "file",
    file_mtime_ns: 0,
    backlink_count: 0,
    forward_link_count: 0,
  };
}

/** A fetch double recording the URL it was called with, answering `body`. */
function stubFetch(
  status: number,
  body: unknown,
): { fetch: typeof fetch; calls: string[] } {
  const calls: string[] = [];
  const fetchImpl = vi.fn((input: RequestInfo | URL) => {
    calls.push(String(input));
    const response = new Response(JSON.stringify(body), {
      status,
      headers: { "content-type": "application/json" },
    });
    return Promise.resolve(response);
  });
  return { fetch: fetchImpl as unknown as typeof fetch, calls };
}

describe("ReadingClient URL construction", () => {
  it("passes a slipbox key as an encoded query parameter, never a path segment", async () => {
    const { fetch, calls } = stubFetch(200, { backlinks: [] });
    const client = new ReadingClient("", fetch);

    await client.backlinks("heading:notes/alpha.org:12", { limit: 5, unique: true });

    expect(calls).toHaveLength(1);
    const url = calls[0]!;
    expect(url).toBe(
      "/api/backlinks?key=heading%3Anotes%2Falpha.org%3A12&limit=5&unique=true",
    );
  });

  it("omits undefined optional parameters from the query string", async () => {
    const { fetch, calls } = stubFetch(200, { nodes: [] });
    const client = new ReadingClient("", fetch);

    await client.searchNodes("alpha");

    expect(calls[0]).toBe("/api/search/nodes?q=alpha");
  });

  it("threads optional parameters through when provided", async () => {
    const { fetch, calls } = stubFetch(200, { nodes: [] });
    const client = new ReadingClient("", fetch);

    await client.searchNodes("alpha", { limit: 10, sort: "backlink-count" });

    expect(calls[0]).toBe("/api/search/nodes?q=alpha&limit=10&sort=backlink-count");
  });

  it("addresses metadata search and content search as separate routes", async () => {
    const { fetch, calls } = stubFetch(200, { hits: [] });
    const client = new ReadingClient("", fetch);

    await client.searchContent("gradient descent", { limit: 10 });

    expect(calls[0]).toBe("/api/search/content?q=gradient+descent&limit=10");
  });

  it("joins paths against a non-empty base without a doubled slash", async () => {
    const { fetch, calls } = stubFetch(200, { status: "ok" });
    const client = new ReadingClient("http://127.0.0.1:8080/", fetch);

    await client.healthz();

    expect(calls[0]).toBe("http://127.0.0.1:8080/api/healthz");
  });
});

describe("ReadingClient exploration reads", () => {
  it("carries the lens on the query string beside the key", async () => {
    const { fetch, calls } = stubFetch(200, { lens: "time", sections: [] });
    const client = new ReadingClient("", fetch);

    await client.explore("file:alpha.org", "time", { limit: 20 });

    expect(calls[0]).toBe("/api/explore?key=file%3Aalpha.org&lens=time&limit=20");
  });

  it("leaves the limit to the route when none is given", async () => {
    const { fetch, calls } = stubFetch(200, { lens: "structure", sections: [] });
    const client = new ReadingClient("", fetch);

    await client.explore("file:alpha.org", "structure");

    expect(calls[0]).toBe("/api/explore?key=file%3Aalpha.org&lens=structure");
  });

  it("tells one lens's two sections apart, empty section and all", async () => {
    const body: ExploreResult = {
      lens: "unresolved",
      sections: [
        {
          kind: "unresolved-tasks",
          entries: [
            {
              kind: "anchor",
              anchor: node("heading:notes/a.org:4", "Prove the bound"),
              explanation: {
                kind: "unresolved-shared-reference",
                references: ["cite:shared2024"],
                todo_keyword: "TODO",
              },
            },
          ],
        },
        { kind: "weakly-integrated-notes", entries: [] },
      ],
    };
    const { fetch } = stubFetch(200, body);
    const client = new ReadingClient("", fetch);

    const result = await client.explore("file:alpha.org", "unresolved");

    expect(result.lens).toBe("unresolved");
    expect(result.sections.map((section) => section.kind)).toEqual([
      "unresolved-tasks",
      "weakly-integrated-notes",
    ]);
    // A section that found nothing arrives empty, not missing.
    expect(result.sections[1]!.entries).toEqual([]);
  });

  it("reads an anchor entry's record beside its tag rather than under it", async () => {
    const body: ExploreResult = {
      lens: "bridges",
      sections: [
        {
          kind: "bridge-candidates",
          entries: [
            {
              kind: "anchor",
              anchor: node("file:beta.org", "Beta"),
              explanation: {
                kind: "bridge-candidate",
                references: ["cite:shared2024"],
                via_notes: [
                  { node_key: "file:weak.org", explicit_id: null, title: "Weak" },
                ],
              },
            },
          ],
        },
      ],
    };
    const { fetch } = stubFetch(200, body);
    const client = new ReadingClient("", fetch);

    const result = await client.explore("file:alpha.org", "bridges");

    // Serde flattens the record into the tagged object.
    const [entry] = result.sections[0]!.entries;
    if (entry?.kind !== "anchor") {
      throw new Error(`expected an anchor entry, got \`${entry?.kind}\``);
    }
    expect(entry.anchor.title).toBe("Beta");
    if (entry.explanation.kind !== "bridge-candidate") {
      throw new Error(`expected a bridge candidate, got \`${entry.explanation.kind}\``);
    }
    expect(entry.explanation.via_notes.map((via) => via.title)).toEqual(["Weak"]);
  });

  it("decodes an explanation written before it carried its evidence notes", async () => {
    // `via_notes` is defaulted in slipbox-core, so a payload predating it decodes.
    const body = {
      lens: "dormant",
      sections: [
        {
          kind: "dormant-notes",
          entries: [
            {
              kind: "anchor",
              anchor: node("file:old.org", "Old"),
              explanation: {
                kind: "dormant-shared-reference",
                references: ["cite:shared2024"],
                modified_at_ns: 1,
              },
            },
          ],
        },
      ],
    };
    const { fetch } = stubFetch(200, body);
    const client = new ReadingClient("", fetch);

    const result = await client.explore("file:alpha.org", "dormant");

    const [entry] = result.sections[0]!.entries;
    if (entry?.kind !== "anchor") {
      throw new Error(`expected an anchor entry, got \`${entry?.kind}\``);
    }
    if (entry.explanation.kind !== "dormant-shared-reference") {
      throw new Error(`expected a dormant note, got \`${entry.explanation.kind}\``);
    }
    expect(entry.explanation.references).toEqual(["cite:shared2024"]);
    expect(entry.explanation.modified_at_ns).toBe(1);
    expect(entry.anchor.title).toBe("Old");
  });
});

describe("ReadingClient unlinked-reference reads", () => {
  it("carries the key and the scan bound on the query string", async () => {
    const { fetch, calls } = stubFetch(200, { unlinked_references: [] });
    const client = new ReadingClient("", fetch);

    await client.unlinkedReferences("heading:notes/alpha.org::12", { limit: 200 });

    expect(calls[0]).toBe(
      "/api/unlinked-references?key=heading%3Anotes%2Falpha.org%3A%3A12&limit=200",
    );
  });

  it("reads a mention's anchor, its position, and the text matched", async () => {
    const body: UnlinkedReferencesResult = {
      unlinked_references: [
        {
          source_anchor: node("heading:notes/beta.org::4", "Beta"),
          row: 12,
          col: 7,
          preview: "where Alpha is named without a link",
          matched_text: "Alpha",
          explanation: { kind: "unlinked-reference", matched_text: "Alpha" },
        },
      ],
    };
    const { fetch } = stubFetch(200, body);
    const client = new ReadingClient("", fetch);

    const result = await client.unlinkedReferences("file:alpha.org");

    const [record] = result.unlinked_references;
    expect(record?.source_anchor.node_key).toBe("heading:notes/beta.org::4");
    expect(record?.col).toBe(7);
    // The column indexes the preview, counting characters from 1.
    expect(record?.preview.slice(record.col - 1)).toMatch(/^Alpha/);
    expect(record?.explanation.kind).toBe("unlinked-reference");
  });
});

describe("ReadingClient error handling", () => {
  it("maps a JSON error envelope to a typed ApiError", async () => {
    const { fetch } = stubFetch(404, {
      error: { kind: "not-found", message: "no note found for key `x`" },
    });
    const client = new ReadingClient("", fetch);

    const error = await client.nodeByKey("x").catch((caught: unknown) => caught);

    expect(error).toBeInstanceOf(ApiError);
    const apiError = error as ApiError;
    expect(apiError.status).toBe(404);
    expect(apiError.kind).toBe("not-found");
    expect(apiError.isNotFound).toBe(true);
    expect(apiError.message).toBe("no note found for key `x`");
  });

  it("falls back to a synthetic message when the error body is not JSON", async () => {
    const fetchImpl = vi.fn(() =>
      Promise.resolve(new Response("upstream exploded", { status: 502 })),
    );
    const client = new ReadingClient("", fetchImpl as unknown as typeof fetch);

    const error = (await client
      .status()
      .catch((caught: unknown) => caught)) as ApiError;

    expect(error).toBeInstanceOf(ApiError);
    expect(error.status).toBe(502);
    expect(error.message).toContain("502");
  });

  it("returns the parsed body on a successful response", async () => {
    const { fetch } = stubFetch(200, {
      version: "0.17.0",
      root: "/notes",
      db: "/notes/slipbox.sqlite",
      files_indexed: 4,
      nodes_indexed: 4,
      links_indexed: 1,
    });
    const client = new ReadingClient("", fetch);

    const status = await client.status();

    expect(status.version).toBe("0.17.0");
    expect(status.nodes_indexed).toBe(4);
  });
});
