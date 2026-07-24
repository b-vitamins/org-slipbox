import { describe, expect, it, vi } from "vitest";

import { ApiError, ReadingClient } from "./client.js";

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
