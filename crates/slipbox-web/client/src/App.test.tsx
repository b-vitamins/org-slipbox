import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App.js";
import type { NodeRecord } from "./api/types.js";
import { __resetRefocusForTests } from "./data/refetch-on-focus.js";
import { SCHEME_ATTRIBUTE } from "./dom/color-scheme.js";
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

function glossaryTerm(title: string): NodeRecord {
  return {
    node_key: `notes/${title}.org::0`,
    explicit_id: null,
    file_path: `notes/${title}.org`,
    title,
    outline_path: title,
    aliases: [],
    tags: [],
    refs: [],
    todo_keyword: null,
    scheduled_for: null,
    deadline_for: null,
    closed_at: null,
    glossary: true,
    glossary_status: "confirmed",
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

function noteContext(title: string): unknown {
  const note = glossaryTerm(title);
  return {
    note,
    source: {
      file_path: note.file_path,
      start_line: 1,
      line_count: 1,
      total_lines: 1,
      content: `The definition of ${title}.`,
      truncated_before: false,
      truncated_after: false,
    },
    node_start_line: 1,
    node_line_count: 1,
    backlinks: [],
    forward_links: [],
  };
}

function routedFetch(routes: Record<string, unknown>): typeof fetch {
  return vi.fn((input: RequestInfo | URL) => {
    const url = typeof input === "string" ? input : input.toString();
    const match = Object.keys(routes)
      .filter((path) => url.startsWith(path))
      .sort((a, b) => b.length - a.length)[0];
    const body = match
      ? routes[match]
      : { error: { kind: "not-found", message: url } };
    return Promise.resolve(
      new Response(JSON.stringify(body), {
        status: match ? 200 : 404,
        headers: { "content-type": "application/json" },
      }),
    );
  }) as unknown as typeof fetch;
}

describe("App shell", () => {
  beforeEach(() => {
    __resetRefocusForTests();
    document.title = BASE_TITLE;
    // The reading stack and surface-view store both read the address bar, so
    // each test starts at the root rather than inheriting the prior one's.
    window.history.replaceState(null, "", "/");
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.title = "";
    // The scheme control writes the document root and localStorage, both of
    // which outlive a render.
    document.documentElement.removeAttribute(SCHEME_ATTRIBUTE);
    window.localStorage.clear();
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
            JSON.stringify({
              error: { kind: "unavailable", message: "daemon is down" },
            }),
            { status: 503, headers: { "content-type": "application/json" } },
          ),
        ),
      ),
    );

    render(() => <App />);

    expect(
      await screen.findByText("unavailable: daemon is down"),
    ).toBeInTheDocument();
  });

  it("toggles from the note entry to the glossary dictionary in the empty frame", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/glossary/terms": { terms: [] },
      }),
    );

    render(() => <App />);

    expect(
      await screen.findByRole("combobox", { name: "Search notes" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Glossary" }));

    expect(
      await screen.findByRole("tab", { name: "Due for review" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("combobox", { name: "Search notes" }),
    ).not.toBeInTheDocument();
  });

  it("reloads into the glossary's review list the URL names", async () => {
    window.history.replaceState(null, "", "?view=review");
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/glossary/terms": { terms: [glossaryTerm("Alpha")] },
        "/api/glossary/due": { terms: [glossaryTerm("Due term")] },
        "/api/note/context": noteContext("Due term"),
      }),
    );

    render(() => <App />);

    expect(
      await screen.findByRole("option", { name: /Due term/ }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Due for review" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("button", { name: "Glossary" })).toHaveAttribute(
      "aria-current",
      "true",
    );
  });

  it("shows the browse list when the Glossary tab is pressed from the review list", async () => {
    window.history.replaceState(null, "", "?view=review");
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/glossary/terms": { terms: [glossaryTerm("Alpha")] },
        "/api/glossary/due": { terms: [glossaryTerm("Due term")] },
        "/api/note/context": noteContext("Alpha"),
      }),
    );

    render(() => <App />);
    await screen.findByRole("option", { name: /Due term/ });

    fireEvent.click(screen.getByRole("button", { name: "Glossary" }));

    expect(
      await screen.findByRole("option", { name: "Alpha" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "All terms" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(window.location.search).toBe("?view=glossary");
  });

  it("reloads a shared glossary search onto its matches", async () => {
    window.history.replaceState(null, "", "?q=entropy&view=glossary");
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/glossary/terms": { terms: [glossaryTerm("Alpha")] },
        "/api/glossary/search": { terms: [glossaryTerm("Entropy")] },
        "/api/note/context": noteContext("Entropy"),
      }),
    );

    render(() => <App />);

    const field = await screen.findByRole("combobox", {
      name: "Search the glossary",
    });
    expect(field).toHaveValue("entropy");
    expect(
      await screen.findByRole("option", { name: "Entropy" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Alpha" }),
    ).not.toBeInTheDocument();
  });

  it("keeps the term and the surface mode as one query, neither writer clobbering the other", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": { hits: [] },
        "/api/glossary/terms": { terms: [glossaryTerm("Alpha")] },
        "/api/glossary/search": { terms: [glossaryTerm("Entropy")] },
        "/api/glossary/due": { terms: [] },
        "/api/note/context": noteContext("Entropy"),
      }),
    );

    render(() => <App />);

    fireEvent.input(
      await screen.findByRole("combobox", { name: "Search notes" }),
      { target: { value: "entropy" } },
    );
    await waitFor(() => expect(window.location.search).toBe("?q=entropy"));

    fireEvent.click(screen.getByRole("button", { name: "Glossary" }));
    expect(window.location.search).toBe("?q=entropy&view=glossary");
    expect(
      await screen.findByRole("combobox", { name: "Search the glossary" }),
    ).toHaveValue("entropy");

    fireEvent.click(screen.getByRole("tab", { name: "Due for review" }));
    await screen.findByRole("heading", { name: "How terms come due" });
    expect(window.location.search).toBe("?q=entropy&view=review");

    fireEvent.click(screen.getByRole("tab", { name: "All terms" }));
    fireEvent.input(
      await screen.findByRole("combobox", { name: "Search the glossary" }),
      { target: { value: "prior" } },
    );
    await waitFor(() =>
      expect(window.location.search).toBe("?q=prior&view=glossary"),
    );
  });

  it("gives a reading view an in-app exit back to the entry", async () => {
    // Nothing but /api/status is routed, so the note is a dead end: the case
    // where the reader's only way out is the surface's own exit.
    window.history.replaceState(null, "", "?note=notes/missing.org::0");
    vi.stubGlobal("fetch", routedFetch({ "/api/status": status }));

    render(() => <App />);

    const home = await screen.findByRole("button", { name: "slipbox" });
    expect(
      screen.queryByRole("combobox", { name: "Search notes" }),
    ).not.toBeInTheDocument();

    fireEvent.click(home);

    expect(
      await screen.findByRole("combobox", { name: "Search notes" }),
    ).toBeInTheDocument();
  });

  it("cycles the color scheme from the header and keeps the control across the frame", async () => {
    // Starting in the spine exercises the control on the reading view and then
    // follows it out to the entry, across a frame swap.
    window.history.replaceState(null, "", "?note=notes/missing.org::0");
    vi.stubGlobal("fetch", routedFetch({ "/api/status": status }));

    render(() => <App />);

    // Auto is the absence of the override attribute.
    const control = await screen.findByRole("button", {
      name: "Color scheme: Auto",
    });
    expect(document.documentElement.hasAttribute(SCHEME_ATTRIBUTE)).toBe(false);

    fireEvent.click(control);
    expect(
      await screen.findByRole("button", { name: "Color scheme: Light" }),
    ).toBeInTheDocument();
    expect(document.documentElement.getAttribute(SCHEME_ATTRIBUTE)).toBe("light");

    fireEvent.click(control);
    expect(
      await screen.findByRole("button", { name: "Color scheme: Dark" }),
    ).toBeInTheDocument();
    expect(document.documentElement.getAttribute(SCHEME_ATTRIBUTE)).toBe("dark");

    fireEvent.click(screen.getByRole("button", { name: "slipbox" }));

    expect(
      await screen.findByRole("combobox", { name: "Search notes" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Color scheme: Dark" }),
    ).toBeInTheDocument();
    expect(document.documentElement.getAttribute(SCHEME_ATTRIBUTE)).toBe("dark");
  });
});
