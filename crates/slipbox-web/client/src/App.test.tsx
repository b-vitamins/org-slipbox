import { render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App.js";
import { __resetRefocusForTests } from "./data/refetch-on-focus.js";
import { BASE_TITLE } from "./dom/document-title.js";

const status = {
  version: "0.17.0",
  root: "/home/reader/notes",
  db: "/home/reader/notes/slipbox.sqlite",
  files_indexed: 4,
  nodes_indexed: 4,
  links_indexed: 1,
};

describe("App shell", () => {
  beforeEach(() => {
    __resetRefocusForTests();
    document.title = BASE_TITLE;
  });

  afterEach(() => {
    vi.restoreAllMocks();
    document.title = "";
  });

  it("reads /api/status and renders the served identity end to end", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(JSON.stringify(status), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
        ),
      ),
    );

    render(() => <App />);

    expect(await screen.findByText("/home/reader/notes")).toBeInTheDocument();
    expect(await screen.findByText("slipbox 0.17.0")).toBeInTheDocument();
    expect(fetch).toHaveBeenCalledWith(
      "/api/status",
      expect.objectContaining({ method: "GET" }),
    );

    vi.unstubAllGlobals();
  });

  it("surfaces the error envelope when the surface is unreachable", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(
            JSON.stringify({ error: { kind: "unavailable", message: "daemon is down" } }),
            { status: 503, headers: { "content-type": "application/json" } },
          ),
        ),
      ),
    );

    render(() => <App />);

    expect(await screen.findByText("unavailable: daemon is down")).toBeInTheDocument();
    expect(document.title).toBe("Unavailable — slipbox");

    vi.unstubAllGlobals();
  });

  it("exposes the async region as a polite live region for assistive tech", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(JSON.stringify(status), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
        ),
      ),
    );

    const { container } = render(() => <App />);
    const main = container.querySelector("main");

    expect(main).toHaveAttribute("aria-live", "polite");
    await screen.findByText("/home/reader/notes");

    vi.unstubAllGlobals();
  });
});
