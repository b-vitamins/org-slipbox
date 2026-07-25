import { fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { NeighborhoodRings } from "./NeighborhoodRings.jsx";
import { NavigationProvider, type Navigation } from "../org/navigation.jsx";
import { __resetRefocusForTests } from "../data/refetch-on-focus.js";
import type { NodeRecord } from "../api/types.js";

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

function jsonRoute(body: unknown): typeof fetch {
  return vi.fn(() =>
    Promise.resolve(
      new Response(JSON.stringify(body), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    ),
  ) as unknown as typeof fetch;
}

const inertNav: Navigation = { glance: () => {}, pin: () => {}, go: () => {} };

function mount(nodeKey = "notes/self.org::0", shown?: ReadonlySet<string>) {
  return render(() => (
    <NavigationProvider navigation={inertNav}>
      <NeighborhoodRings nodeKey={nodeKey} shown={shown} />
    </NavigationProvider>
  ));
}

describe("NeighborhoodRings", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("does not fetch until the disclosure is opened", () => {
    const fetchImpl = jsonRoute({
      origin: "notes/self.org::0",
      hops: 2,
      nodes: [],
      edges: [],
      truncated: false,
    });
    vi.stubGlobal("fetch", fetchImpl);

    mount();

    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it("walks the neighborhood on open and lists it in distance rings", async () => {
    vi.stubGlobal(
      "fetch",
      jsonRoute({
        origin: "notes/self.org::0",
        hops: 2,
        nodes: [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
          { node: node("notes/b.org::0", "Beta"), distance: 2 },
        ],
        edges: [],
        truncated: false,
      }),
    );

    mount();
    fireEvent.click(
      screen.getByRole("button", { name: "Explore neighborhood" }),
    );

    expect(await screen.findByText("1 hop away")).toBeInTheDocument();
    expect(screen.getByText("2 hops away")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Alpha" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Beta" })).toBeInTheDocument();
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining(
        "/api/neighborhood?key=notes%2Fself.org%3A%3A0&hops=2",
      ),
      expect.objectContaining({ method: "GET" }),
    );
  });

  it("omits neighbors already shown as immediate relations", async () => {
    vi.stubGlobal(
      "fetch",
      jsonRoute({
        origin: "notes/self.org::0",
        hops: 2,
        nodes: [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
          { node: node("notes/b.org::0", "Beta"), distance: 2 },
        ],
        edges: [],
        truncated: false,
      }),
    );

    mount("notes/self.org::0", new Set(["notes/a.org::0"]));
    fireEvent.click(
      screen.getByRole("button", { name: "Explore neighborhood" }),
    );

    expect(
      await screen.findByRole("link", { name: "Beta" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "Alpha" }),
    ).not.toBeInTheDocument();
  });

  it("reports a walk that hit the node ceiling", async () => {
    vi.stubGlobal(
      "fetch",
      jsonRoute({
        origin: "notes/self.org::0",
        hops: 2,
        nodes: [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
        ],
        edges: [],
        truncated: true,
      }),
    );

    mount();
    fireEvent.click(
      screen.getByRole("button", { name: "Explore neighborhood" }),
    );

    expect(
      await screen.findByText(
        "Showing the nearest notes; the neighborhood is larger.",
      ),
    ).toBeInTheDocument();
  });

  it("reports an isolated note", async () => {
    vi.stubGlobal(
      "fetch",
      jsonRoute({
        origin: "notes/self.org::0",
        hops: 2,
        nodes: [{ node: node("notes/self.org::0", "Self"), distance: 0 }],
        edges: [],
        truncated: false,
      }),
    );

    mount();
    fireEvent.click(
      screen.getByRole("button", { name: "Explore neighborhood" }),
    );

    expect(
      await screen.findByText("This note has no neighbors."),
    ).toBeInTheDocument();
  });

  it("shows the nearest of a wide ring and offers the rest", async () => {
    vi.stubGlobal(
      "fetch",
      jsonRoute({
        origin: "notes/self.org::0",
        hops: 2,
        nodes: [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          ...Array.from({ length: 12 }, (_, index) => ({
            node: node(`notes/n${index}.org::0`, `Note ${index}`),
            distance: 1,
          })),
        ],
        edges: [],
        truncated: false,
      }),
    );

    mount();
    fireEvent.click(
      screen.getByRole("button", { name: "Explore neighborhood" }),
    );

    expect(await screen.findByRole("link", { name: "Note 0" })).toBeInTheDocument();
    expect(screen.getAllByRole("link")).toHaveLength(8);
    expect(screen.queryByRole("link", { name: "Note 8" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Show 4 more" }));
    expect(screen.getByRole("link", { name: "Note 11" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Show \d+ more/ })).not.toBeInTheDocument();
  });

  it("explains a far member by the nearer notes it is reached through", async () => {
    vi.stubGlobal(
      "fetch",
      jsonRoute({
        origin: "notes/self.org::0",
        hops: 2,
        nodes: [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
          { node: node("notes/far.org::0", "Far"), distance: 2 },
        ],
        edges: [
          { source: "notes/a.org::0", target: "notes/far.org::0", kind: "forward" },
        ],
        truncated: false,
      }),
    );

    mount();
    fireEvent.click(
      screen.getByRole("button", { name: "Explore neighborhood" }),
    );

    expect(await screen.findByText("via Alpha")).toBeInTheDocument();
  });

  it("filters the rings by title, reaching past the shown bound", async () => {
    vi.stubGlobal(
      "fetch",
      jsonRoute({
        origin: "notes/self.org::0",
        hops: 2,
        nodes: [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          ...Array.from({ length: 11 }, (_, index) => ({
            node: node(`notes/n${index}.org::0`, `Note ${index}`),
            distance: 1,
          })),
          { node: node("notes/duality.org::0", "Duality gap"), distance: 1 },
        ],
        edges: [],
        truncated: false,
      }),
    );

    mount();
    fireEvent.click(
      screen.getByRole("button", { name: "Explore neighborhood" }),
    );
    await screen.findByRole("link", { name: "Note 0" });
    // Sent last and equally connected, so it sits below the cut.
    expect(screen.queryByRole("link", { name: "Duality gap" })).not.toBeInTheDocument();

    const field = screen.getByRole("searchbox", { name: "Filter the neighborhood" });
    fireEvent.input(field, { target: { value: "duality" } });

    expect(screen.getByRole("link", { name: "Duality gap" })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Note 0" })).not.toBeInTheDocument();

    fireEvent.input(field, { target: { value: "nothing here" } });
    expect(
      screen.getByText("No nearby notes match that filter."),
    ).toBeInTheDocument();
  });

  it("offers no filter over a neighborhood short enough to read whole", async () => {
    vi.stubGlobal(
      "fetch",
      jsonRoute({
        origin: "notes/self.org::0",
        hops: 2,
        nodes: [
          { node: node("notes/self.org::0", "Self"), distance: 0 },
          { node: node("notes/a.org::0", "Alpha"), distance: 1 },
        ],
        edges: [],
        truncated: false,
      }),
    );

    mount();
    fireEvent.click(
      screen.getByRole("button", { name: "Explore neighborhood" }),
    );

    expect(await screen.findByRole("link", { name: "Alpha" })).toBeInTheDocument();
    expect(
      screen.queryByRole("searchbox", { name: "Filter the neighborhood" }),
    ).not.toBeInTheDocument();
  });

  it("surfaces a read error and toggles closed again", async () => {
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

    mount();
    const toggle = screen.getByRole("button", { name: "Explore neighborhood" });
    fireEvent.click(toggle);

    expect(await screen.findByText("daemon is down")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Hide neighborhood" }));
    expect(screen.queryByText("daemon is down")).not.toBeInTheDocument();
  });
});
