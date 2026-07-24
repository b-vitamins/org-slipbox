import { render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App.js";
import type { NodeRecord } from "./api/types.js";
import { __resetRefocusForTests } from "./data/refetch-on-focus.js";
import { BASE_TITLE } from "./dom/document-title.js";

const status = {
  version: "0.17.0",
  root: "/home/reader/notes",
  db: "/home/reader/notes/slipbox.sqlite",
  files_indexed: 4,
  nodes_indexed: 4,
  notes_indexed: 4,
  links_indexed: 1,
};

describe("App shell", () => {
  beforeEach(() => {
    __resetRefocusForTests();
    document.title = BASE_TITLE;
    // The reading stack reads the address bar; start each test at the root.
    window.history.replaceState(null, "", "/");
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.title = "";
  });

  it("reads /api/status and rests on the entry surface with the served identity", async () => {
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

    expect(
      await screen.findByRole("combobox", { name: "Search notes" }),
    ).toBeInTheDocument();
    expect(await screen.findByText("slipbox 0.17.0")).toBeInTheDocument();
    expect(screen.queryByText("/home/reader/notes")).not.toBeInTheDocument();
    expect(fetch).toHaveBeenCalledWith(
      "/api/status",
      expect.objectContaining({ method: "GET" }),
    );
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
  });
});
