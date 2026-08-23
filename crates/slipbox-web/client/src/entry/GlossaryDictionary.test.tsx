import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { createSignal, type Accessor } from "solid-js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  GlossaryDictionary,
  type GlossaryMode,
} from "./GlossaryDictionary.jsx";
import type { QueryUrl } from "./query-url.js";
import { __resetRefocusForTests } from "../data/refetch-on-focus.js";
import type { NodeRecord } from "../api/types.js";

function term(
  key: string,
  title: string,
  extra: Partial<NodeRecord> = {},
): NodeRecord {
  return {
    node_key: key,
    explicit_id: null,
    file_path: "notes/g.org",
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
    ...extra,
  };
}

function contextFor(key: string, title: string, body: string): unknown {
  return {
    note: term(key, title),
    source: {
      file_path: "notes/g.org",
      start_line: 1,
      line_count: body.split("\n").length,
      total_lines: body.split("\n").length,
      content: body,
      truncated_before: false,
      truncated_after: false,
    },
    node_start_line: 1,
    node_line_count: body.split("\n").length,
    backlinks: [],
    forward_links: [],
  };
}

function ok(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

function requestedKey(url: string): string | null {
  return new URL(url, "http://slipbox.test").searchParams.get("key");
}

function routedFetch(routes: Record<string, unknown>): typeof fetch {
  return vi.fn((input: RequestInfo | URL) => {
    const url = typeof input === "string" ? input : input.toString();
    // Longest matching prefix wins, so `/api/glossary/search` beats `/api/glossary`.
    const match = Object.keys(routes)
      .filter((path) => url.startsWith(path))
      .sort((a, b) => b.length - a.length)[0];
    if (match) {
      return Promise.resolve(
        new Response(JSON.stringify(routes[match]), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      );
    }
    return Promise.resolve(
      new Response(
        JSON.stringify({ error: { kind: "not-found", message: url } }),
        {
          status: 404,
          headers: { "content-type": "application/json" },
        },
      ),
    );
  }) as unknown as typeof fetch;
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

/**
 * Mode lives in a signal so a test can both start in a mode and read back the
 * one the surface reports. The `?q=` seam defaults to in-memory and the
 * debounce to zero, so no test writes the real address bar or advances a timer.
 */
function mount(
  options: {
    onOpen?: (key: string) => void;
    mode?: GlossaryMode;
    debounceMs?: number;
    queryUrl?: QueryUrl;
  } = {},
): {
  readonly mode: Accessor<GlossaryMode>;
  readonly showMode: (next: GlossaryMode) => void;
} {
  const [mode, setMode] = createSignal<GlossaryMode>(options.mode ?? "browse");
  const queryUrl = options.queryUrl ?? memoryQueryUrl();
  render(() => (
    <GlossaryDictionary
      onOpen={options.onOpen ?? (() => {})}
      mode={mode()}
      onMode={setMode}
      debounceMs={options.debounceMs ?? 0}
      queryUrl={queryUrl}
    />
  ));
  return { mode, showMode: (next) => setMode(next) };
}

describe("GlossaryDictionary", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("lists every term on entry and peeks the first automatically", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/entropy.org::0", "Entropy"),
            term("notes/prior.org::0", "Prior"),
          ],
        },
        "/api/note/context": contextFor(
          "notes/entropy.org::0",
          "Entropy",
          "A measure of uncertainty.",
        ),
      }),
    );

    mount();

    const options = await screen.findAllByRole("option");
    expect(options).toHaveLength(2);
    expect(
      await screen.findByText("A measure of uncertainty."),
    ).toBeInTheDocument();
  });

  it("heads the surface with one top-level heading above the headword", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [term("notes/entropy.org::0", "Entropy")],
        },
        "/api/note/context": contextFor(
          "notes/entropy.org::0",
          "Entropy",
          "* Facets\n\nProse.\n\n** Deeper\n\nMore prose.",
        ),
      }),
    );

    mount();
    await screen.findByText("Prose.");

    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(
      screen.getByRole("heading", { level: 1, name: "Glossary" }),
    ).toBeInTheDocument();
    // The headword outranks the definition's own headings, and the two Org
    // levels below it skip nothing on the way down.
    expect(
      screen.getByRole("heading", { level: 2, name: "Entropy" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 3, name: "Facets" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 4, name: "Deeper" }),
    ).toBeInTheDocument();
  });

  it("keeps the surface heading when the glossary holds no terms", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/glossary/terms": { terms: [] } }),
    );

    mount();
    await screen.findByText("The glossary has no terms yet.");

    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(
      screen.getByRole("heading", { level: 1, name: "Glossary" }),
    ).toBeInTheDocument();
  });

  it("keeps the surface heading when the glossary cannot be read", async () => {
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
    await screen.findByText("unavailable: daemon is down");

    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(
      screen.getByRole("heading", { level: 1, name: "Glossary" }),
    ).toBeInTheDocument();
  });

  it("searches the glossary as the reader types", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/search": {
          terms: [term("notes/entropy.org::0", "Entropy")],
        },
        "/api/note/context": contextFor(
          "notes/entropy.org::0",
          "Entropy",
          "Body.",
        ),
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.input(screen.getByRole("combobox"), {
      target: { value: "entro" },
    });

    expect(
      await screen.findByRole("option", { name: "Entropy" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Alpha" }),
    ).not.toBeInTheDocument();
  });

  it("switches to the due list in study mode and hides the search box", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": {
          terms: [
            term("notes/due.org::0", "Due term", {
              sr_due: "2026-07-20",
              sr_reps: "3",
            }),
          ],
        },
        "/api/note/context": contextFor(
          "notes/due.org::0",
          "Due term",
          "Body.",
        ),
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.click(screen.getByRole("tab", { name: "Due for review" }));

    expect(
      await screen.findByRole("option", { name: /Due term/ }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    expect(await screen.findByLabelText("Review schedule")).toBeInTheDocument();
    expect(screen.getByText("2026-07-20")).toBeInTheDocument();
  });

  it("reports an empty due list", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [] },
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.click(screen.getByRole("tab", { name: "Due for review" }));

    expect(
      await screen.findByText("Nothing is due for review."),
    ).toBeInTheDocument();
  });

  it("opens the peeked term in the reader from a control beside its headword", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [term("notes/entropy.org::0", "Entropy")],
        },
        "/api/note/context": contextFor(
          "notes/entropy.org::0",
          "Entropy",
          "Body.",
        ),
      }),
    );

    mount({ onOpen });
    await screen.findByText("Body.");

    const control = screen.getByRole("link", { name: "Open in reader" });
    // Beside the headword, not below the definition: a definition long enough to
    // scroll would carry the way onward off the bottom of the pane.
    expect(control.closest(".glossary-peek__header")).not.toBeNull();
    // An anchor rather than a button, so the destination is one a reader can
    // copy or open in a new tab.
    expect(control).toHaveAttribute("href", "?note=notes%2Fentropy.org%3A%3A0");

    fireEvent.click(control);
    expect(onOpen).toHaveBeenCalledWith("notes/entropy.org::0");
  });

  it("names an id-addressable term by its id rather than its file key", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/entropy.org::0", "Entropy", {
              explicit_id: "entropy-uuid",
            }),
          ],
        },
        "/api/note/context": contextFor(
          "notes/entropy.org::0",
          "Entropy",
          "Body.",
        ),
      }),
    );

    mount({ onOpen });
    await screen.findByText("Body.");

    // An id reference survives a rename, so the control opens the term the same
    // way a link to it would.
    const control = screen.getByRole("link", { name: "Open in reader" });
    expect(control).toHaveAttribute("href", "?note=id%3Aentropy-uuid");

    fireEvent.click(control);
    expect(onOpen).toHaveBeenCalledWith("id:entropy-uuid");
  });

  it("follows a link inside a definition to the note it names", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [term("notes/entropy.org::0", "Entropy")],
        },
        "/api/note/context": contextFor(
          "notes/entropy.org::0",
          "Entropy",
          "Dual to [[id:prior-uuid][the prior]] over the same events.",
        ),
      }),
    );

    mount({ onOpen });

    const link = await screen.findByRole("link", { name: "the prior" });
    // The anchor advertises a real destination, so it must honor one: a link the
    // surface renders live and then swallows is worse than inert text.
    expect(link).toHaveAttribute("href", "?note=id%3Aprior-uuid");

    fireEvent.click(link);
    expect(onOpen).toHaveBeenCalledWith("id:prior-uuid");
  });

  it("opens a term on Enter over the highlighted row", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/a.org::0", "Alpha"),
            term("notes/b.org::0", "Beta"),
          ],
        },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    mount({ onOpen });
    await screen.findAllByRole("option");

    const field = screen.getByRole("combobox");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "Enter" });

    expect(onOpen).toHaveBeenCalledWith("notes/b.org::0");
  });

  it("keeps the peeked term across a re-read of the same list", async () => {
    const onOpen = vi.fn();
    const listed = [term("notes/a.org::0", "Alpha"), term("notes/b.org::0", "Beta")];
    let asked = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL) => {
        const url = String(input);
        if (url.startsWith("/api/glossary/terms")) {
          const terms =
            asked++ === 0 ? listed : [term("notes/aa.org::0", "Aardvark"), ...listed];
          return Promise.resolve(ok({ terms }));
        }
        const key = requestedKey(url) ?? "";
        return Promise.resolve(ok(contextFor(key, key, "Body.")));
      }) as unknown as typeof fetch,
    );

    mount({ onOpen });
    await screen.findByRole("option", { name: "Beta" });

    const field = screen.getByRole("combobox");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(field).toHaveAttribute("aria-activedescendant", "glossary-option-1");

    fireEvent(window, new Event("visibilitychange"));
    fireEvent(window, new Event("focus"));
    await screen.findByRole("option", { name: "Aardvark" });

    await waitFor(() =>
      expect(field).toHaveAttribute("aria-activedescendant", "glossary-option-2"),
    );
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onOpen).toHaveBeenCalledExactlyOnceWith("notes/b.org::0");
  });

  it("never opens a term for a search the field has already replaced", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/search": { terms: [term("notes/b.org::0", "Beta")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    // A debounce no timer in this test advances: only Enter settles the query.
    mount({ onOpen, debounceMs: 100_000 });
    await screen.findByRole("option", { name: "Alpha" });

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "beta" } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onOpen).not.toHaveBeenCalled();

    expect(await screen.findByRole("option", { name: "Beta" })).toBeInTheDocument();
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onOpen).toHaveBeenCalledExactlyOnceWith("notes/b.org::0");
  });

  it("waits for a searchable word instead of showing the server's refusal", async () => {
    const fetch = routedFetch({
      "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
      "/api/glossary/search": { terms: [term("notes/b.org::0", "Beta")] },
      "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
    });
    vi.stubGlobal("fetch", fetch);

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    const field = screen.getByRole("combobox");
    fireEvent.input(field, { target: { value: "b" } });
    await Promise.resolve();

    const searched = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls.some(
      ([url]) => String(url).includes("/api/glossary/search"),
    );
    expect(searched).toBe(false);
    expect(document.querySelector(".glossary-status--error")).toBeNull();
    expect(document.querySelector(".glossary-status--hint")?.textContent).toBe(
      "Searching needs a word of at least 2 characters.",
    );

    fireEvent.input(field, { target: { value: "be" } });
    expect(await screen.findByRole("option", { name: "Beta" })).toBeInTheDocument();
    expect(document.querySelector(".glossary-status--hint")).toBeNull();
  });

  it("navigates the due list by keyboard when the search box is gone", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": {
          terms: [
            term("notes/one.org::0", "One"),
            term("notes/two.org::0", "Two"),
          ],
        },
        "/api/note/context": contextFor("notes/one.org::0", "One", "Body."),
      }),
    );

    mount({ onOpen });
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.click(screen.getByRole("tab", { name: "Due for review" }));
    await screen.findByRole("option", { name: "One" });

    const listbox = screen.getByRole("listbox");
    expect(listbox).toHaveAttribute("tabindex", "0");
    fireEvent.keyDown(listbox, { key: "ArrowDown" });
    fireEvent.keyDown(listbox, { key: "Enter" });

    expect(onOpen).toHaveBeenCalledWith("notes/two.org::0");
  });

  it("surfaces a glossary read error", async () => {
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

    expect(
      await screen.findByText("unavailable: daemon is down"),
    ).toBeInTheDocument();
  });

  it("marks a stub term in both the list and the peek", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/stub.org::0", "Stubby", { glossary_status: "stub" }),
          ],
        },
        "/api/note/context": contextFor("notes/stub.org::0", "Stubby", "Body."),
      }),
    );

    mount();

    await screen.findByText("Body.");
    // Two badges name the stub: one on the list row, one in the peek.
    await waitFor(() =>
      expect(screen.getAllByText("stub").length).toBeGreaterThanOrEqual(2),
    );
  });

  it("peeks a term on click rather than opening it, so touch can use the peek", async () => {
    const onOpen = vi.fn();
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/a.org::0", "Alpha"),
            term("notes/b.org::0", "Beta"),
          ],
        },
        "/api/note/context": contextFor("notes/b.org::0", "Beta", "Beta body."),
      }),
    );

    mount({ onOpen });
    const options = await screen.findAllByRole("option");

    fireEvent.click(options[1]!);

    expect(options[1]!).toHaveAttribute("aria-selected", "true");
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("moves between glossary modes with the arrow keys", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    mount();
    const browseTab = await screen.findByRole("tab", { name: "All terms" });
    const studyTab = screen.getByRole("tab", { name: "Due for review" });

    expect(browseTab).toHaveAttribute("aria-selected", "true");
    expect(studyTab).toHaveAttribute("tabindex", "-1");

    fireEvent.keyDown(browseTab, { key: "ArrowRight" });

    expect(studyTab).toHaveAttribute("aria-selected", "true");
    expect(
      await screen.findByRole("option", { name: /Due term/ }),
    ).toBeInTheDocument();

    fireEvent.keyDown(studyTab, { key: "ArrowLeft" });
    expect(browseTab).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(browseTab, { key: "ArrowLeft" });
    expect(studyTab).toHaveAttribute("aria-selected", "true");
  });

  it("leaves the vertical arrows to the term list the tabs sit above", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    mount();
    const browseTab = await screen.findByRole("tab", { name: "All terms" });
    const studyTab = screen.getByRole("tab", { name: "Due for review" });

    expect(screen.getByRole("tablist")).toHaveAttribute(
      "aria-orientation",
      "horizontal",
    );

    fireEvent.keyDown(browseTab, { key: "ArrowDown" });
    expect(browseTab).toHaveAttribute("aria-selected", "true");
    expect(studyTab).toHaveAttribute("aria-selected", "false");

    fireEvent.keyDown(browseTab, { key: "ArrowUp" });
    expect(browseTab).toHaveAttribute("aria-selected", "true");
  });

  it("names an empty glossary rather than blaming an absent search", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/glossary/terms": { terms: [] } }),
    );

    mount();

    expect(
      await screen.findByText("The glossary has no terms yet."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("No terms match that search."),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("Select a term to read its definition."),
    ).not.toBeInTheDocument();
  });

  it("explains how a note becomes a term when the glossary is empty", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({ "/api/glossary/terms": { terms: [] } }),
    );

    mount();

    expect(
      await screen.findByRole("heading", {
        name: "How terms enter the glossary",
      }),
    ).toBeInTheDocument();
    expect(screen.getByText("#+glossary: t")).toBeInTheDocument();
    expect(screen.getByText("slipbox glossary mark")).toBeInTheDocument();
    expect(
      screen.getByText("org-slipbox-glossary-define"),
    ).toBeInTheDocument();
  });

  it("explains the review schedule when nothing is due", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [] },
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.click(screen.getByRole("tab", { name: "Due for review" }));

    expect(
      await screen.findByRole("heading", { name: "How terms come due" }),
    ).toBeInTheDocument();
    expect(screen.getByText("slipbox glossary grade")).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "How terms enter the glossary" }),
    ).not.toBeInTheDocument();
  });

  it("sends a fruitless glossary search on to the wider note search", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/search": { terms: [] },
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "zzz" } });

    expect(
      await screen.findByText(/the Notes surface searches every note/),
    ).toBeInTheDocument();
  });

  it("says nothing about an empty glossary while the read is still in flight", async () => {
    let land!: (body: unknown) => void;
    vi.stubGlobal(
      "fetch",
      vi.fn(
        () => new Promise<Response>((resolve) => {
          land = (body) => resolve(ok(body));
        }),
      ) as unknown as typeof fetch,
    );

    mount();

    await waitFor(() => expect(land).toBeTypeOf("function"));
    expect(
      screen.queryByRole("heading", { name: "How terms enter the glossary" }),
    ).not.toBeInTheDocument();

    land({ terms: [term("notes/a.org::0", "Alpha")] });
    await screen.findByRole("option", { name: "Alpha" });
    expect(
      screen.queryByRole("heading", { name: "How terms enter the glossary" }),
    ).not.toBeInTheDocument();
  });

  it("names the glossary in the browser tab, and its review mode apart", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    expect(document.title).toBe("Glossary — slipbox");

    fireEvent.click(screen.getByRole("tab", { name: "Due for review" }));
    await screen.findByRole("heading", { name: "How terms come due" });
    expect(document.title).toBe("Glossary review — slipbox");
  });

  it("never shows the previous term's definition under a new term's title", async () => {
    // Each definition fetch parks until a release[] entry answers it, which
    // holds the surface in the in-between state long enough to inspect.
    const release: (() => void)[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL) => {
        const url = typeof input === "string" ? input : input.toString();
        if (url.startsWith("/api/glossary/terms")) {
          return Promise.resolve(
            ok({
              terms: [
                term("notes/entropy.org::0", "Entropy"),
                term("notes/prior.org::0", "Prior"),
              ],
            }),
          );
        }
        const key = requestedKey(url) ?? "";
        const body =
          key === "notes/entropy.org::0"
            ? contextFor(key, "Entropy", "A measure of uncertainty.")
            : contextFor(key, "Prior", "A belief before evidence.");
        return new Promise<Response>((resolve) => {
          release.push(() => resolve(ok(body)));
        });
      }) as unknown as typeof fetch,
    );

    mount();
    const options = await screen.findAllByRole("option");

    await waitFor(() => expect(release).toHaveLength(1));
    release[0]!();
    await screen.findByText("A measure of uncertainty.");

    fireEvent.mouseEnter(options[1]!);

    await screen.findByRole("heading", { name: "Prior" });
    expect(
      screen.queryByText("A measure of uncertainty."),
    ).not.toBeInTheDocument();

    await waitFor(() => expect(release).toHaveLength(2));
    release[1]!();
    expect(
      await screen.findByText("A belief before evidence."),
    ).toBeInTheDocument();
  });

  it("scrolls the term cursor into view", async () => {
    // jsdom implements no scrollIntoView, so the call needs a stub to record.
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/a.org::0", "Alpha"),
            term("notes/b.org::0", "Beta"),
            term("notes/c.org::0", "Gamma"),
          ],
        },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    mount();
    await screen.findAllByRole("option");
    scrollIntoView.mockClear();

    fireEvent.keyDown(screen.getByRole("combobox"), { key: "ArrowDown" });

    expect(scrollIntoView.mock.instances.at(-1)).toBe(
      screen.getByRole("option", { name: "Beta" }),
    );
  });

  it("keeps the term list navigable past a definition it cannot render", async () => {
    // A 200 whose context carries no source is what makes the renderer throw.
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL) => {
        const url = typeof input === "string" ? input : input.toString();
        if (url.startsWith("/api/glossary/terms")) {
          return Promise.resolve(
            ok({
              terms: [
                term("notes/broken.org::0", "Broken"),
                term("notes/prior.org::0", "Prior"),
              ],
            }),
          );
        }
        const key = requestedKey(url) ?? "";
        return Promise.resolve(
          key === "notes/broken.org::0"
            ? ok({
                note: term(key, "Broken"),
                node_start_line: 1,
                node_line_count: 1,
                backlinks: [],
                forward_links: [],
              })
            : ok(contextFor(key, "Prior", "A belief before evidence.")),
        );
      }) as unknown as typeof fetch,
    );

    mount();

    expect(
      await screen.findByText("This definition could not be rendered."),
    ).toBeInTheDocument();
    const options = await screen.findAllByRole("option");
    expect(options).toHaveLength(2);

    fireEvent.keyDown(screen.getByRole("combobox"), { key: "ArrowDown" });

    expect(
      await screen.findByText("A belief before evidence."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("This definition could not be rendered."),
    ).not.toBeInTheDocument();
  });

  it("reports a search that matches no terms", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/search": { terms: [] },
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "zzz" } });

    expect(
      await screen.findByText("No terms match that search."),
    ).toBeInTheDocument();
  });

  it("renders the list the mode it is given names, without being pressed", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/due.org::0", "Due term", "Body."),
      }),
    );

    mount({ mode: "study" });

    expect(
      await screen.findByRole("option", { name: /Due term/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("tab", { name: "Due for review" }),
    ).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Alpha" }),
    ).not.toBeInTheDocument();
  });

  it("reports the mode the reader picks rather than switching lists itself", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    const surface = mount();
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.click(screen.getByRole("tab", { name: "Due for review" }));
    expect(surface.mode()).toBe("study");

    fireEvent.click(screen.getByRole("tab", { name: "All terms" }));
    expect(surface.mode()).toBe("browse");
  });

  it("takes a mode arriving from outside as a list change of its own", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/a.org::0", "Alpha"),
            term("notes/shared.org::0", "Shared"),
          ],
        },
        "/api/glossary/due": {
          terms: [
            term("notes/one.org::0", "One"),
            term("notes/shared.org::0", "Shared"),
          ],
        },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    const surface = mount();
    await screen.findByRole("option", { name: "Shared" });
    fireEvent.mouseEnter(screen.getByRole("option", { name: "Shared" }));
    expect(screen.getByRole("option", { name: "Shared" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    surface.showMode("study");

    await screen.findByRole("option", { name: "One" });
    expect(screen.getByRole("option", { name: "One" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("option", { name: "Shared" })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("moves focus to the tab a mode arriving from outside selects", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    const surface = mount({ mode: "study" });
    const browseTab = await screen.findByRole("tab", { name: "All terms" });
    const studyTab = screen.getByRole("tab", { name: "Due for review" });
    studyTab.focus();
    expect(document.activeElement).toBe(studyTab);

    surface.showMode("browse");

    expect(document.activeElement).toBe(browseTab);
    expect(browseTab).toHaveAttribute("aria-selected", "true");
    expect(browseTab.tabIndex).toBe(0);
    expect(studyTab).toHaveAttribute("aria-selected", "false");
    expect(studyTab).toHaveAttribute("tabindex", "-1");
  });

  it("leaves focus where it is when the mode moves and the tablist is not holding it", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    const surface = mount();
    const browseTab = await screen.findByRole("tab", { name: "All terms" });
    const studyTab = screen.getByRole("tab", { name: "Due for review" });
    const field = screen.getByRole("combobox");
    field.focus();
    expect(document.activeElement).toBe(field);

    surface.showMode("study");
    await screen.findByRole("option", { name: /Due term/ });

    // Removing the focused search box drops focus to the body, not to a tab.
    expect(document.activeElement).toBe(document.body);
    expect(studyTab).toHaveAttribute("aria-selected", "true");
    expect(studyTab.tabIndex).toBe(0);
    expect(browseTab).toHaveAttribute("tabindex", "-1");

    surface.showMode("browse");
    await screen.findByRole("option", { name: "Alpha" });

    expect(document.activeElement).toBe(document.body);
  });

  it("seeds its search from a restored ?q= and mirrors later terms back", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/search": {
          terms: [term("notes/entropy.org::0", "Entropy")],
        },
        "/api/note/context": contextFor("notes/entropy.org::0", "Entropy", "Body."),
      }),
    );
    const queryUrl = memoryQueryUrl("entropy");

    mount({ queryUrl });

    expect(
      await screen.findByRole("option", { name: "Entropy" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toHaveValue("entropy");
    expect(
      screen.queryByRole("option", { name: "Alpha" }),
    ).not.toBeInTheDocument();

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "prior" } });
    fireEvent.input(screen.getByRole("combobox"), { target: { value: "" } });
    expect(queryUrl.writes).toEqual(["prior", null]);
  });

  it("holds its search term while the reader looks at what is due", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/search": {
          terms: [term("notes/entropy.org::0", "Entropy")],
        },
        "/api/glossary/due": { terms: [] },
        "/api/note/context": contextFor("notes/entropy.org::0", "Entropy", "Body."),
      }),
    );
    const queryUrl = memoryQueryUrl();

    const surface = mount({ queryUrl });
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "entropy" } });
    await screen.findByRole("option", { name: "Entropy" });

    fireEvent.click(screen.getByRole("tab", { name: "Due for review" }));
    await screen.findByRole("heading", { name: "How terms come due" });

    expect(queryUrl.writes).toEqual(["entropy"]);
    expect(queryUrl.read()).toBe("entropy");

    surface.showMode("browse");
    expect(
      await screen.findByRole("option", { name: "Entropy" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toHaveValue("entropy");
  });
});
