import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { CursorHistory } from "./cursor-history.js";
import { EntrySurface } from "./EntrySurface.jsx";
import type { QueryUrl } from "./query-url.js";
import { __resetRefocusForTests } from "../data/refetch-on-focus.js";
import { BASE_TITLE } from "../dom/document-title.js";
import type { NodeContentHit, NodeRecord } from "../api/types.js";

const status = {
  version: "0.17.0",
  root: "/home/reader/notes",
  db: "/home/reader/notes/slipbox.sqlite",
  files_indexed: 554,
  nodes_indexed: 1036,
  notes_indexed: 560,
  links_indexed: 2222,
};

function node(key: string, title: string, tags: string[]): NodeRecord {
  return {
    node_key: key,
    explicit_id: null,
    file_path: "notes/x.org",
    title,
    outline_path: title,
    aliases: [],
    tags,
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

function hit(
  record: NodeRecord,
  segments: NodeContentHit["snippet"]["segments"] = [],
): NodeContentHit {
  return { node: record, snippet: { segments } };
}

function hits(count: number): NodeContentHit[] {
  return Array.from({ length: count }, (_, i) =>
    hit(node(`notes/n${i}.org::0`, `Note ${i}`, [])),
  );
}

function routedFetch(routes: Record<string, unknown>): typeof fetch {
  return vi.fn((input: RequestInfo | URL) => {
    const url = typeof input === "string" ? input : input.toString();
    for (const [path, body] of Object.entries(routes)) {
      if (url.startsWith(path)) {
        return Promise.resolve(
          new Response(JSON.stringify(body), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
        );
      }
    }
    return Promise.resolve(
      new Response(JSON.stringify({ error: { kind: "not-found", message: url } }), {
        status: 404,
        headers: { "content-type": "application/json" },
      }),
    );
  }) as unknown as typeof fetch;
}

/** Let an in-flight search settle, so anything it would open has opened by now. */
function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function memoryQueryUrl(initial: string | null = null): QueryUrl & {
  readonly writes: (string | null)[];
} {
  let current = initial;
  const writes: (string | null)[] = [];
  return {
    read: () => current,
    replace: (term) => {
      current = term;
      writes.push(term);
    },
    writes,
  };
}

function memoryCursorHistory(initial: string | null = null): CursorHistory & {
  readonly writes: (string | null)[];
} {
  let current = initial;
  const writes: (string | null)[] = [];
  return {
    read: () => current,
    write: (key) => {
      current = key;
      writes.push(key);
    },
    writes,
  };
}

describe("EntrySurface", () => {
  beforeEach(() => {
    __resetRefocusForTests();
    document.title = BASE_TITLE;
    // jsdom keeps one history entry for a whole file, so a test using the real
    // cursor seam would otherwise read what the test before it recorded.
    window.history.replaceState(null, "", "/");
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.title = "";
  });

  it("rests on the served identity before anything is searched", async () => {
    vi.stubGlobal("fetch", routedFetch({ "/api/status": status }));

    render(() => <EntrySurface onOpen={() => {}} queryUrl={memoryQueryUrl()} />);

    expect(await screen.findByText("notes")).toBeInTheDocument();
    expect(screen.queryByText("/home/reader/notes")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1 })).toHaveAttribute(
      "title",
      "/home/reader/notes",
    );
    expect(screen.getByText(/560 notes/)).toBeInTheDocument();
    expect(screen.queryByText(/1036 notes/)).not.toBeInTheDocument();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("searches as the reader types and lists matching notes with their tags", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [
            hit(node("notes/kmeans.org::0", "K-means objective", ["optimization"])),
            hit(node("notes/gibbs.org::0", "Gibbs sampling", ["sampling"])),
          ],
        },
      }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "means" } });

    const options = await screen.findAllByRole("option");
    expect(options).toHaveLength(2);
    expect(screen.getByText("K-means objective")).toBeInTheDocument();
    expect(screen.getByText("optimization")).toBeInTheDocument();
  });

  it("searches note prose, not just titles and aliases", async () => {
    const fetch = routedFetch({
      "/api/status": status,
      "/api/search/content": {
        hits: [
          hit(node("notes/duality.org::0", "Lagrangian duality", []), [
            { text: "the dual of a ", matched: false },
            { text: "global maximization", matched: true },
            { text: " problem", matched: false },
          ]),
        ],
      },
    });
    vi.stubGlobal("fetch", fetch);

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), {
      target: { value: "global maximization" },
    });
    await screen.findByText("Lagrangian duality");

    const asked = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls.map(([url]) =>
      String(url),
    );
    expect(asked.some((url) => url.startsWith("/api/search/content?q="))).toBe(true);
    expect(asked.some((url) => url.startsWith("/api/search/nodes"))).toBe(false);

    const excerpt = document.querySelector(".entry-result__snippet");
    expect(excerpt?.textContent).toBe("the dual of a global maximization problem");
    expect(excerpt?.querySelector("mark")?.textContent).toBe("global maximization");
  });

  it("renders an excerpt as prose, not as the Org source it matched", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [
            hit(node("notes/collapse.org::0", "Collapse", []), [
              { text: "/Posterior collapse/ afflicts [[id:abc][", matched: false },
              { text: "autoencoders", matched: true },
              { text: "]] alike", matched: false },
            ]),
          ],
        },
      }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "autoencoders" } });
    await screen.findByText("Collapse");

    const excerpt = document.querySelector(".entry-result__snippet");
    expect(excerpt?.textContent).toBe("Posterior collapse afflicts autoencoders alike");
    expect(excerpt?.querySelector("em")?.textContent).toBe("Posterior collapse");
    expect(excerpt?.querySelector("mark")?.textContent).toBe("autoencoders");
    expect(excerpt?.querySelector("a")).toBeNull();
  });

  it("renders an excerpt as text, so prose that looks like markup stays prose", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [
            hit(node("notes/markup.org::0", "Markup", []), [
              { text: "<mark>not a highlight</mark> and ", matched: false },
              { text: "markup", matched: true },
            ]),
          ],
        },
      }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "markup" } });
    await screen.findByText("Markup");

    const excerpt = document.querySelector(".entry-result__snippet");
    expect(excerpt?.textContent).toBe("<mark>not a highlight</mark> and markup");
    expect(excerpt?.querySelectorAll("mark")).toHaveLength(1);
    expect(excerpt?.querySelector("mark")?.textContent).toBe("markup");
  });

  it("never searches an empty query", async () => {
    const fetch = routedFetch({ "/api/status": status });
    vi.stubGlobal("fetch", fetch);

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "kmeans" } });
    fireEvent.input(field, { target: { value: "" } });

    // Let any errant deferred fetch flush.
    await Promise.resolve();
    const searched = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls.some(
      ([url]) => String(url).includes("/api/search/content"),
    );
    const blankSearch = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls.some(
      ([url]) => String(url).includes("/api/search/content?q=&"),
    );
    expect(searched).toBe(true);
    expect(blankSearch).toBe(false);
  });

  it("waits for a searchable word instead of showing the server's refusal", async () => {
    const fetch = routedFetch({
      "/api/status": status,
      "/api/search/content": { hits: [hit(node("notes/kmeans.org::0", "K-means", []))] },
    });
    vi.stubGlobal("fetch", fetch);

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "k" } });
    await Promise.resolve();

    const calls = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls;
    expect(calls.some(([url]) => String(url).includes("/api/search/content"))).toBe(
      false,
    );
    expect(document.querySelector(".entry-status--error")).toBeNull();
    expect(document.querySelector(".entry-status--hint")?.textContent).toBe(
      "Searching needs a word of at least 2 characters.",
    );

    fireEvent.input(field, { target: { value: "km" } });
    expect(await screen.findByText("K-means")).toBeInTheDocument();
    expect(document.querySelector(".entry-status--hint")).toBeNull();
  });

  it("opens the arrow-highlighted result on Enter", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [
            hit(node("notes/kmeans.org::0", "K-means objective", ["optimization"])),
            hit(node("notes/gibbs.org::0", "Gibbs sampling", ["sampling"])),
          ],
        },
      }),
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "sampling" } });
    await screen.findAllByRole("option");

    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "Enter" });

    expect(onOpen).toHaveBeenCalledWith("notes/gibbs.org::0");
  });

  it("scrolls the arrow cursor into view", async () => {
    // jsdom implements no scrollIntoView, so the call needs a stub to record.
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": { hits: hits(8) },
      }),
    );

    render(() => (
      <EntrySurface
        onOpen={() => {}}
        debounceMs={0}
        queryUrl={memoryQueryUrl()}
      />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "note" } });
    await screen.findAllByRole("option");
    scrollIntoView.mockClear();

    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "ArrowDown" });

    expect(scrollIntoView).toHaveBeenCalled();
    expect(scrollIntoView.mock.instances.at(-1)).toBe(
      screen.getByRole("option", { name: /Note 1/ }),
    );
  });

  it("keeps the arrow cursor across a refetch of the same search", async () => {
    const onOpen = vi.fn();
    const first = [
      hit(node("notes/kmeans.org::0", "K-means objective", [])),
      hit(node("notes/gibbs.org::0", "Gibbs sampling", [])),
    ];
    const afterIndexing = [hit(node("notes/mcmc.org::0", "MCMC", [])), ...first];
    let asked = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL) => {
        const url = String(input);
        const body = url.startsWith("/api/search/content")
          ? { hits: asked++ === 0 ? first : afterIndexing }
          : status;
        return Promise.resolve(
          new Response(JSON.stringify(body), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
        );
      }) as unknown as typeof fetch,
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "sampling" } });
    await screen.findByText("Gibbs sampling");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(field).toHaveAttribute("aria-activedescendant", "entry-option-1");

    fireEvent(window, new Event("visibilitychange"));
    fireEvent(window, new Event("focus"));
    await screen.findByText("MCMC");

    await waitFor(() =>
      expect(field).toHaveAttribute("aria-activedescendant", "entry-option-2"),
    );
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onOpen).toHaveBeenCalledExactlyOnceWith("notes/gibbs.org::0");
  });

  it("drops the arrow cursor when a new search no longer lists what it marked", async () => {
    const onOpen = vi.fn();
    const answers = [
      [hit(node("notes/kmeans.org::0", "K-means objective", []))],
      [hit(node("notes/gibbs.org::0", "Gibbs sampling", []))],
    ];
    let asked = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL) => {
        const url = String(input);
        const body = url.startsWith("/api/status")
          ? status
          : { hits: answers[Math.min(asked++, answers.length - 1)] };
        return Promise.resolve(
          new Response(JSON.stringify(body), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
        );
      }) as unknown as typeof fetch,
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "means" } });
    await screen.findByText("K-means objective");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(field).toHaveAttribute("aria-activedescendant", "entry-option-0");

    fireEvent.input(field, { target: { value: "gibbs" } });
    await screen.findByText("Gibbs sampling");
    expect(field).not.toHaveAttribute("aria-activedescendant");
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onOpen).toHaveBeenCalledWith("notes/gibbs.org::0");
  });

  it("opens the top match when Enter is pressed with nothing highlighted", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [
            hit(node("notes/kmeans.org::0", "K-means objective", ["optimization"])),
            hit(node("notes/gibbs.org::0", "Gibbs sampling", ["sampling"])),
          ],
        },
      }),
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "means" } });
    await screen.findAllByRole("option");

    fireEvent.keyDown(field, { key: "Enter" });

    expect(onOpen).toHaveBeenCalledWith("notes/kmeans.org::0");
  });

  it("never opens a note for a query the field has already replaced", async () => {
    const onOpen = vi.fn();
    // One note per query, so a stale result set is recognizable by name.
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL) => {
        const url = typeof input === "string" ? input : input.toString();
        const body = url.startsWith("/api/status")
          ? status
          : {
              hits: url.includes("q=alpha")
                ? [hit(node("notes/alpha.org::0", "Alpha", []))]
                : [hit(node("notes/beta.org::0", "Beta", []))],
            };
        return Promise.resolve(
          new Response(JSON.stringify(body), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
        );
      }) as unknown as typeof fetch,
    );

    // A debounce no timer in this test advances: only Enter settles the query.
    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={100_000} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "alpha" } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(await screen.findByRole("option", { name: /Alpha/ })).toBeInTheDocument();
    await waitFor(() =>
      expect(onOpen).toHaveBeenCalledExactlyOnceWith("notes/alpha.org::0"),
    );

    fireEvent.input(field, { target: { value: "beta" } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onOpen).toHaveBeenCalledTimes(1);

    expect(await screen.findByRole("option", { name: /Beta/ })).toBeInTheDocument();
    await waitFor(() => expect(onOpen).toHaveBeenCalledTimes(2));
    expect(onOpen).toHaveBeenLastCalledWith("notes/beta.org::0");
  });

  it("searches and opens the top match from a single Enter", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [
            hit(node("notes/proto.org::0", "Prototypical maximum principle", [])),
            hit(node("notes/weak.org::0", "Weak maximum principle", [])),
          ],
        },
      }),
    );

    // A debounce no timer in this test advances: only Enter settles the query.
    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={100_000} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "proto maximum principle" } });
    fireEvent.keyDown(field, { key: "Enter" });

    await waitFor(() =>
      expect(onOpen).toHaveBeenCalledExactlyOnceWith("notes/proto.org::0"),
    );
  });

  it("abandons the Enter's open when the reader types on before it lands", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": { hits: [hit(node("notes/alpha.org::0", "Alpha", []))] },
      }),
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={100_000} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "alpha" } });
    fireEvent.keyDown(field, { key: "Enter" });
    fireEvent.input(field, { target: { value: "alphabet" } });

    expect(await screen.findByRole("option", { name: /Alpha/ })).toBeInTheDocument();
    await flush();
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("abandons the Enter's open when Escape follows it", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": { hits: [hit(node("notes/alpha.org::0", "Alpha", []))] },
      }),
    );

    // A window short enough to elapse on its own, so the search after the
    // Escape settles with no second Enter to arm anything.
    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={20} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "alpha" } });
    fireEvent.keyDown(field, { key: "Enter" });
    fireEvent.keyDown(field, { key: "Escape" });
    await flush();
    expect(field).toHaveValue("");
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
    expect(onOpen).not.toHaveBeenCalled();

    fireEvent.input(field, { target: { value: "alpha" } });
    expect(await screen.findByRole("option", { name: /Alpha/ })).toBeInTheDocument();
    await flush();
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("abandons the Enter's open when the arrow keys move the cursor", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": { hits: [hit(node("notes/alpha.org::0", "Alpha", []))] },
      }),
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={100_000} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "alpha" } });
    fireEvent.keyDown(field, { key: "Enter" });
    fireEvent.keyDown(field, { key: "ArrowDown" });

    expect(await screen.findByRole("option", { name: /Alpha/ })).toBeInTheDocument();
    await flush();
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("opens nothing when the Enter's own search matches no note", async () => {
    const onOpen = vi.fn();
    // The same term answered twice: nothing, and then a note indexed since.
    let asked = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL) => {
        const url = String(input);
        const body = url.startsWith("/api/search/content")
          ? { hits: asked++ === 0 ? [] : [hit(node("notes/zeta.org::0", "Zeta", []))] }
          : status;
        return Promise.resolve(
          new Response(JSON.stringify(body), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
        );
      }) as unknown as typeof fetch,
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={100_000} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "zzz" } });
    fireEvent.keyDown(field, { key: "Enter" });

    expect(await screen.findByText("No notes match that search.")).toBeInTheDocument();
    expect(onOpen).not.toHaveBeenCalled();

    fireEvent(window, new Event("visibilitychange"));
    fireEvent(window, new Event("focus"));
    expect(await screen.findByRole("option", { name: /Zeta/ })).toBeInTheDocument();
    await flush();
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("opens nothing when the Enter's own search fails", async () => {
    const onOpen = vi.fn();
    // The first read of the term fails; a re-read of it succeeds.
    const failure = { error: { kind: "unavailable", message: "index is busy" } };
    const found = { hits: [hit(node("notes/eta.org::0", "Eta", []))] };
    let asked = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL) => {
        const searching = String(input).startsWith("/api/search/content");
        const failing = searching && asked++ === 0;
        const body = searching ? (failing ? failure : found) : status;
        return Promise.resolve(
          new Response(JSON.stringify(body), {
            status: failing ? 503 : 200,
            headers: { "content-type": "application/json" },
          }),
        );
      }) as unknown as typeof fetch,
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={100_000} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "eta" } });
    fireEvent.keyDown(field, { key: "Enter" });

    expect(await screen.findByText("unavailable: index is busy")).toBeInTheDocument();
    expect(onOpen).not.toHaveBeenCalled();

    fireEvent(window, new Event("visibilitychange"));
    fireEvent(window, new Event("focus"));
    expect(await screen.findByRole("option", { name: /Eta/ })).toBeInTheDocument();
    await flush();
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("opens a note by click", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [hit(node("notes/kmeans.org::0", "K-means objective", ["optimization"]))],
        },
      }),
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "means" } });
    const option = await screen.findByRole("option");
    fireEvent.click(option);

    expect(onOpen).toHaveBeenCalledWith("notes/kmeans.org::0");
  });

  it("notes when the result list is capped, so it never reads as exhaustive", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        // A full page of results: the server may have withheld more.
        "/api/search/content": { hits: hits(20) },
      }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "note" } });
    await screen.findAllByRole("option");

    expect(await screen.findByText(/Showing the first 20 matches/)).toBeInTheDocument();
  });

  it("does not show the cap notice for a partial page of results", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/status": status, "/api/search/content": { hits: hits(3) } }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "note" } });
    await screen.findAllByRole("option");

    expect(screen.queryByText(/Showing the first/)).not.toBeInTheDocument();
  });

  it("opens a random note from the surprise-me control", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/random": { node: node("notes/surprise.org::0", "A surprise", []) },
      }),
    );

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.click(screen.getByRole("button", { name: "Surprise me" }));

    await waitFor(() => expect(onOpen).toHaveBeenCalledWith("notes/surprise.org::0"));
  });

  it("disables surprise-me while a random open is in flight and ignores re-presses", async () => {
    const onOpen = vi.fn();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const random = { node: node("notes/surprise.org::0", "A surprise", []) };
    const gatedFetch = vi.fn((input: RequestInfo | URL): Promise<Response> => {
      const url = typeof input === "string" ? input : input.toString();
      if (url.startsWith("/api/random")) {
        return gate.then(
          () =>
            new Response(JSON.stringify(random), {
              status: 200,
              headers: { "content-type": "application/json" },
            }),
        );
      }
      return Promise.resolve(
        new Response(JSON.stringify(status), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      );
    });
    vi.stubGlobal("fetch", gatedFetch as unknown as typeof fetch);

    render(() => (
      <EntrySurface onOpen={onOpen} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    const button = screen.getByRole("button", { name: "Surprise me" });
    fireEvent.click(button);
    await waitFor(() => expect(button).toBeDisabled());
    fireEvent.click(button);

    release();
    await waitFor(() => expect(onOpen).toHaveBeenCalledTimes(1));
  });

  it("reports a failed random open instead of swallowing it", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/status": status }), // /api/random falls through to 404
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.click(screen.getByRole("button", { name: "Surprise me" }));

    expect(await screen.findByText(/not-found/)).toBeInTheDocument();
  });

  it("reports search that resolves to nothing", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/status": status, "/api/search/content": { hits: [] } }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "zzz" } });

    expect(await screen.findByText("No notes match that search.")).toBeInTheDocument();
  });

  it("seeds its search from a restored ?q= and mirrors later terms back", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [hit(node("notes/kmeans.org::0", "K-means objective", []))],
        },
      }),
    );
    const queryUrl = memoryQueryUrl("means");

    render(() => <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={queryUrl} />);

    expect(await screen.findByText("K-means objective")).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toHaveValue("means");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "gibbs" } });
    fireEvent.input(screen.getByRole("combobox"), { target: { value: "" } });
    expect(queryUrl.writes).toEqual(["gibbs", null]);
  });

  it("owns the tab title, reflecting an unreachable surface", async () => {
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

    render(() => <EntrySurface onOpen={() => {}} queryUrl={memoryQueryUrl()} />);

    expect(await screen.findByText("unavailable: daemon is down")).toBeInTheDocument();
    expect(document.title).toBe("Unavailable — slipbox");
  });

  it("announces from a status line placed before any search, not from the surface", async () => {
    vi.stubGlobal("fetch", routedFetch({ "/api/status": status }));

    const { container } = render(() => (
      <EntrySurface onOpen={() => {}} queryUrl={memoryQueryUrl()} />
    ));
    const main = container.querySelector("main");

    expect(main).not.toHaveAttribute("aria-live");
    // The surface announced nothing once the region moved off it, so it reports
    // no busy state either.
    expect(main).not.toHaveAttribute("aria-busy");

    // Placed and empty: a region announces nothing it already held when it
    // arrived, so it cannot be mounted with the first answer in it.
    const region = screen.getByRole("status");
    expect(region.textContent).toBe("");

    expect(await screen.findByText("notes")).toBeInTheDocument();
    expect(region).not.toContainElement(screen.getByRole("heading", { level: 1 }));
  });

  it("announces how many notes matched, and holds no result row", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/status": status, "/api/search/content": { hits: hits(3) } }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "note" } });
    await screen.findAllByRole("option");

    const region = screen.getByRole("status");
    expect(region.textContent).toBe("3 notes match.");
    expect(region).not.toContainElement(screen.getByRole("listbox"));
    expect(region.querySelectorAll('[role="option"]')).toHaveLength(0);
  });

  it("counts a single match in the singular", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/status": status, "/api/search/content": { hits: hits(1) } }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "note" } });
    await screen.findByRole("option");

    expect(screen.getByRole("status").textContent).toBe("1 note matches.");
  });

  it("announces a search that matched nothing", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/status": status, "/api/search/content": { hits: [] } }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "zzz" } });

    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe("No notes match that search."),
    );
  });

  it("states the count once, leaving the capped list the advice", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/status": status, "/api/search/content": { hits: hits(20) } }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "note" } });
    await screen.findAllByRole("option");

    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe(
        "Showing the first 20 matches.",
      ),
    );
    const more = document.querySelector(".entry-status--more")?.textContent;
    expect(more).toBe("Refine your search to narrow it.");
  });

  it("does not re-announce a list that re-renders under the same query", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/status": status, "/api/search/content": { hits: hits(3) } }),
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "note" } });
    await screen.findAllByRole("option");
    const region = screen.getByRole("status");
    expect(region.textContent).toBe("3 notes match.");

    const changes: MutationRecord[] = [];
    const observer = new MutationObserver((records) => changes.push(...records));
    observer.observe(region, { childList: true, characterData: true, subtree: true });

    // A refocus re-reads the same term and answers it with the same notes, which
    // re-renders the list. Nothing in the region may be rewritten.
    fireEvent(window, new Event("visibilitychange"));
    fireEvent(window, new Event("focus"));
    await flush();
    observer.disconnect();

    expect(region.textContent).toBe("3 notes match.");
    expect(changes).toEqual([]);
  });

  it("announces an unreachable surface from the region that stood before it", async () => {
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

    render(() => <EntrySurface onOpen={() => {}} queryUrl={memoryQueryUrl()} />);
    // Taken while the read is still out, so the element the failure lands in is
    // the one that was already there to announce it.
    const region = screen.getByRole("status");

    await waitFor(() =>
      expect(region.textContent).toBe("unavailable: daemon is down"),
    );
    expect(region).toBeInTheDocument();
  });

  it("announces a failed random open", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/status": status }), // /api/random falls through to 404
    );

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");
    const region = screen.getByRole("status");

    fireEvent.click(screen.getByRole("button", { name: "Surprise me" }));

    await waitFor(() => expect(region.textContent).toMatch(/not-found/));
  });

  it("announces that a search is waiting on a word long enough to run", async () => {
    vi.stubGlobal("fetch", routedFetch({ "/api/status": status }));

    render(() => (
      <EntrySurface onOpen={() => {}} debounceMs={0} queryUrl={memoryQueryUrl()} />
    ));
    await screen.findByText("notes");
    const region = screen.getByRole("status");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "k" } });

    await waitFor(() =>
      expect(region.textContent).toBe(
        "Searching needs a word of at least 2 characters.",
      ),
    );
  });

  it("focuses the search field on arrival, so a restored search is arrowable", async () => {
    vi.stubGlobal("fetch", routedFetch({ "/api/status": status }));

    render(() => <EntrySurface onOpen={() => {}} queryUrl={memoryQueryUrl()} />);

    expect(screen.getByRole("combobox")).toHaveFocus();
    await screen.findByText("notes");
  });

  it("records the opened result on the history entry it leaves", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [
            hit(node("notes/a.org::0", "First", [])),
            hit(node("notes/b.org::0", "Second", [])),
          ],
        },
      }),
    );
    const cursorHistory = memoryCursorHistory();
    const opened: string[] = [];

    render(() => (
      <EntrySurface
        onOpen={(key) => opened.push(key)}
        debounceMs={0}
        queryUrl={memoryQueryUrl()}
        cursorHistory={cursorHistory}
      />
    ));

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "note" } });
    expect(await screen.findByText("Second")).toBeInTheDocument();
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "Enter" });

    expect(opened).toEqual(["notes/b.org::0"]);
    expect(cursorHistory.writes).toEqual(["notes/b.org::0"]);
  });

  it("restores the recorded cursor as the highlight the results arrive with", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [
            hit(node("notes/a.org::0", "First", [])),
            hit(node("notes/b.org::0", "Second", [])),
          ],
        },
      }),
    );

    render(() => (
      <EntrySurface
        onOpen={() => {}}
        debounceMs={0}
        queryUrl={memoryQueryUrl("note")}
        cursorHistory={memoryCursorHistory("notes/b.org::0")}
      />
    ));

    const second = await screen.findByRole("option", { name: /Second/ });
    expect(second).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("combobox")).toHaveAttribute(
      "aria-activedescendant",
      second.id,
    );
  });

  it("drops a recorded cursor that names no note in the results", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [hit(node("notes/a.org::0", "First", []))],
        },
      }),
    );
    const onOpen = vi.fn();

    render(() => (
      <EntrySurface
        onOpen={onOpen}
        debounceMs={0}
        queryUrl={memoryQueryUrl("note")}
        // A cursor from a search whose answer has changed under it.
        cursorHistory={memoryCursorHistory("notes/gone.org::0")}
      />
    ));

    const first = await screen.findByRole("option", { name: /First/ });
    expect(first).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("combobox")).not.toHaveAttribute("aria-activedescendant");

    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter" });
    expect(onOpen).toHaveBeenCalledWith("notes/a.org::0");
  });

  it("keeps the place in the list when leaving for a note that is not in it", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [
            hit(node("notes/a.org::0", "First", [])),
            hit(node("notes/b.org::0", "Second", [])),
          ],
        },
        "/api/random": { node: node("notes/z.org::0", "Somewhere else", []) },
      }),
    );
    const cursorHistory = memoryCursorHistory();
    const opened: string[] = [];

    render(() => (
      <EntrySurface
        onOpen={(key) => opened.push(key)}
        debounceMs={0}
        queryUrl={memoryQueryUrl()}
        cursorHistory={cursorHistory}
      />
    ));

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "note" } });
    expect(await screen.findByText("Second")).toBeInTheDocument();
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "ArrowDown" });

    fireEvent.click(screen.getByRole("button", { name: "Surprise me" }));
    await waitFor(() => expect(opened).toEqual(["notes/z.org::0"]));
    expect(cursorHistory.writes).toEqual(["notes/b.org::0"]);
  });

  it("records no cursor when leaving a list nothing is highlighted in", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/status": status,
        "/api/search/content": {
          hits: [hit(node("notes/a.org::0", "First", []))],
        },
        "/api/random": { node: node("notes/z.org::0", "Somewhere else", []) },
      }),
    );
    const cursorHistory = memoryCursorHistory();
    const opened: string[] = [];

    render(() => (
      <EntrySurface
        onOpen={(key) => opened.push(key)}
        debounceMs={0}
        queryUrl={memoryQueryUrl()}
        cursorHistory={cursorHistory}
      />
    ));

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "note" } });
    expect(await screen.findByText("First")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Surprise me" }));
    await waitFor(() => expect(opened).toEqual(["notes/z.org::0"]));
    expect(cursorHistory.writes).toEqual([null]);
  });

  it("surfaces the daemon error when the status is unreachable", async () => {
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

    render(() => <EntrySurface onOpen={() => {}} queryUrl={memoryQueryUrl()} />);

    expect(await screen.findByText("unavailable: daemon is down")).toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });
});
