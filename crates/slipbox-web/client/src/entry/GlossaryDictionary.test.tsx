import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { createSignal, type Accessor } from "solid-js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  GlossaryDictionary,
  type GlossaryMode,
} from "./GlossaryDictionary.jsx";
import type { QueryUrl } from "./query-url.js";
import type { TermUrl } from "./term-url.js";
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

/**
 * A double routing on the whole request rather than on its path, since the pages
 * of one listing differ only by their query. `answer` returns a body to serve as a
 * 200, or a `Response` of its own for a failing page.
 */
function pagedFetch(answer: (url: URL) => unknown): typeof fetch {
  return vi.fn((input: RequestInfo | URL) => {
    const answered = answer(new URL(String(input), "http://slipbox.test"));
    return Promise.resolve(
      answered instanceof Response ? answered : ok(answered),
    );
  }) as unknown as typeof fetch;
}

/**
 * A browse listing of four terms served two at a time, plus a definition for
 * whichever term is peeked. `past-beta` is the position the first page hands out,
 * which the second page is served for and for nothing else.
 */
function twoPageWorld(url: URL): unknown {
  if (url.pathname === "/api/glossary/terms") {
    return url.searchParams.get("after") === "past-beta"
      ? {
          terms: [
            term("notes/c.org::0", "Gamma"),
            term("notes/d.org::0", "Delta"),
          ],
          total: 4,
          has_more: false,
          next_position: null,
        }
      : {
          terms: [
            term("notes/a.org::0", "Alpha"),
            term("notes/b.org::0", "Beta"),
          ],
          total: 4,
          has_more: true,
          next_position: "past-beta",
        };
  }
  const key = url.searchParams.get("key") ?? "";
  return contextFor(key, key, "Body.");
}

/**
 * The two listings sharing no rows, plus the index and a definition for Beta: the
 * world a term addressed in one listing and absent from the other is read in.
 */
function twoListWorld(url: URL): unknown {
  switch (url.pathname) {
    case "/api/glossary/due":
      return {
        terms: [term("notes/one.org::0", "One", { sr_due: "2024-01-01" })],
        total: 1,
        has_more: false,
      };
    case "/api/glossary/terms":
      return {
        terms: [term("notes/a.org::0", "Alpha"), term("notes/b.org::0", "Beta")],
        total: 2,
        has_more: false,
      };
    case "/api/glossary/term":
      return { term: term("notes/b.org::0", "Beta") };
    default:
      return contextFor("notes/b.org::0", "Beta", "Beta body.");
  }
}

/** The URLs a `pagedFetch` or `routedFetch` double was asked for, in order. */
function asked(stub: typeof fetch): string[] {
  return (stub as unknown as ReturnType<typeof vi.fn>).mock.calls.map(([url]) =>
    String(url),
  );
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

function memoryTermUrl(initial: string | null = null): TermUrl & {
  readonly writes: (string | null)[];
} {
  let current = initial;
  const writes: (string | null)[] = [];
  return {
    read: () => current,
    replace: (key) => {
      current = key;
      writes.push(key);
    },
    writes,
  };
}

/**
 * Mode lives in a signal so a test can both start in a mode and read back the
 * one the surface reports. Both URL seams default to in-memory and the debounce
 * to zero, so no test writes the real address bar or advances a timer.
 */
function mount(
  options: {
    onOpen?: (key: string) => void;
    mode?: GlossaryMode;
    debounceMs?: number;
    queryUrl?: QueryUrl;
    termUrl?: TermUrl;
  } = {},
): {
  readonly mode: Accessor<GlossaryMode>;
  readonly showMode: (next: GlossaryMode) => void;
  /** The modes the surface asked its owner for, in order: the owner pushes each. */
  readonly modeAsks: GlossaryMode[];
} {
  const [mode, setMode] = createSignal<GlossaryMode>(options.mode ?? "browse");
  const queryUrl = options.queryUrl ?? memoryQueryUrl();
  const termUrl = options.termUrl ?? memoryTermUrl();
  const modeAsks: GlossaryMode[] = [];
  render(() => (
    <GlossaryDictionary
      onOpen={options.onOpen ?? (() => {})}
      mode={mode()}
      onMode={(next) => {
        modeAsks.push(next);
        setMode(next);
      }}
      debounceMs={options.debounceMs ?? 0}
      queryUrl={queryUrl}
      termUrl={termUrl}
    />
  ));
  return { mode, showMode: (next) => setMode(next), modeAsks };
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

  it("switches to the due list in study mode and renames the field to it", async () => {
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
    expect(
      screen.getByRole("combobox", { name: "Search the glossary" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Due for review" }));

    expect(
      await screen.findByRole("option", { name: /Due term/ }),
    ).toBeInTheDocument();
    // The box stays, named for the set it acts on: one field, two stated jobs.
    expect(
      screen.getByRole("combobox", { name: "Filter the terms due for review" }),
    ).toBeInTheDocument();
    expect(await screen.findByLabelText("Review schedule")).toBeInTheDocument();
    expect(screen.getByText("2026-07-20")).toBeInTheDocument();
  });

  it("states the size of the due set from the listing's own total", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        // Three rows of a listing of twelve: the count answers for the listing,
        // not for the page the surface is holding.
        "/api/glossary/due": {
          terms: [
            term("notes/one.org::0", "One"),
            term("notes/two.org::0", "Two"),
            term("notes/three.org::0", "Three"),
          ],
          total: 12,
          has_more: true,
          next_position: "cursor-1",
        },
        "/api/note/context": contextFor("notes/one.org::0", "One", "Body."),
      }),
    );

    mount({ mode: "study" });
    await screen.findByRole("option", { name: /One/ });

    // The order is named rather than reordered: the surface holds one page of a
    // listing it did not sort.
    expect(
      screen.getByText(
        "12 terms are due, in schedule order: never reviewed first, then by due date, then by file path.",
      ),
    ).toBeInTheDocument();
  });

  it("counts one due term in the singular", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": {
          terms: [term("notes/one.org::0", "One")],
          total: 1,
          has_more: false,
          next_position: null,
        },
        "/api/note/context": contextFor("notes/one.org::0", "One", "Body."),
      }),
    );

    mount({ mode: "study" });
    await screen.findByRole("option", { name: /One/ });

    expect(screen.getByText(/^1 term is due,/)).toBeInTheDocument();
  });

  it("gives each due row its standing, and the browse list none", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [term("notes/one.org::0", "One")],
          total: 1,
          has_more: false,
        },
        "/api/glossary/due": {
          terms: [
            term("notes/one.org::0", "One"),
            term("notes/two.org::0", "Two", {
              sr_due: "2026-07-20",
              sr_interval: "6",
              sr_reps: "3",
              sr_ease: "2.5",
              sr_last: "2026-07-14",
            }),
            term("notes/three.org::0", "Three", { sr_reps: "3" }),
          ],
          total: 3,
          has_more: false,
        },
        "/api/note/context": contextFor("notes/one.org::0", "One", "Body."),
      }),
    );

    const surface = mount({ mode: "study" });
    const rows = await screen.findAllByRole("option");

    // A corpus no review has touched carries no dates, so the row says which of
    // the two reasons for being due it is standing on.
    expect(rows[0]!).toHaveTextContent("never reviewed");
    expect(rows[1]!).toHaveTextContent("due 2026-07-20");
    // A drawer a grading part-wrote is the third case, and it says which fact it
    // is missing rather than leaving the aside off the row.
    expect(rows[2]!).toHaveTextContent("no due date recorded");

    // The peek says the same of the term it is showing.
    expect(await screen.findByLabelText("Review schedule")).toHaveTextContent(
      "Never reviewed",
    );

    // Browse lists every term, due or not, so a standing there would be a fact
    // about a schedule the reader is not reading.
    surface.showMode("browse");
    const browsed = await screen.findAllByRole("option");
    expect(browsed[0]!).not.toHaveTextContent("never reviewed");
  });

  it("narrows the due list without reaching past what is due", async () => {
    const fetch = routedFetch({
      "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
      "/api/glossary/due": {
        terms: [
          term("notes/entropy.org::0", "Entropy"),
          term("notes/prior.org::0", "Prior", { aliases: ["Entropic belief"] }),
          term("notes/loss.org::0", "Loss"),
        ],
      },
      // A term the index would answer with and the due list does not hold. The
      // due listing carries no search, so reaching for this route at all would
      // put a term that is not due into the review list.
      "/api/glossary/search": { terms: [term("notes/other.org::0", "Entropy pool")] },
      "/api/note/context": contextFor("notes/entropy.org::0", "Entropy", "Body."),
    });
    vi.stubGlobal("fetch", fetch);

    mount({ mode: "study" });
    // A due row is named by its headword and the standing beside it, so the
    // lookups here match the headword the row leads with.
    await screen.findByRole("option", { name: /^Entropy/ });

    fireEvent.input(
      screen.getByRole("combobox", { name: "Filter the terms due for review" }),
      { target: { value: "entrop" } },
    );

    await waitFor(() =>
      expect(screen.queryByRole("option", { name: /^Loss/ })).not.toBeInTheDocument(),
    );
    // The headword matches outright and the synonym matches for its own term.
    expect(screen.getByRole("option", { name: /^Entropy/ })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /^Prior/ })).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: /^Entropy pool/ }),
    ).not.toBeInTheDocument();
    const searched = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls.some(
      ([url]) => String(url).includes("/api/glossary/search"),
    );
    expect(searched).toBe(false);
  });

  it("says which set a fruitless filter was over, and what it does not reach", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/due.org::0", "Due term", "Body."),
      }),
    );

    mount({ mode: "study" });
    await screen.findByRole("option", { name: /^Due term/ });

    fireEvent.input(
      screen.getByRole("combobox", { name: "Filter the terms due for review" }),
      { target: { value: "zzz" } },
    );

    expect(
      await screen.findByText("No terms due for review match that filter."),
    ).toBeInTheDocument();
    // Terms are due, so the standing explanation of how they come due would be
    // answering a question the reader did not ask.
    expect(
      screen.getByRole("heading", { name: "Nothing due matches that filter" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "How terms come due" }),
    ).not.toBeInTheDocument();
  });

  it("keeps saying nothing is due when a filter is held over an empty due list", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [] },
        "/api/glossary/search": { terms: [term("notes/a.org::0", "Alpha")] },
      }),
    );

    mount({ mode: "study" });
    await screen.findByText("Nothing is due for review.");

    fireEvent.input(
      screen.getByRole("combobox", { name: "Filter the terms due for review" }),
      { target: { value: "alpha" } },
    );

    // Nothing was hidden, so the filter has nothing to answer for.
    expect(screen.getByText("Nothing is due for review.")).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "How terms come due" }),
    ).toBeInTheDocument();
  });

  it("carries the query into whichever list the mode names", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/search": {
          terms: [term("notes/entropy.org::0", "Entropy")],
        },
        "/api/glossary/due": {
          terms: [
            term("notes/entropy.org::0", "Entropy"),
            term("notes/loss.org::0", "Loss"),
          ],
        },
        "/api/note/context": contextFor("notes/entropy.org::0", "Entropy", "Body."),
      }),
    );

    const surface = mount();
    await screen.findByRole("option", { name: "Alpha" });

    const field = screen.getByRole("combobox", { name: "Search the glossary" });
    fireEvent.input(field, { target: { value: "entrop" } });
    await screen.findByRole("option", { name: "Entropy" });

    surface.showMode("study");

    // The query is held rather than dropped, and applies to the list that
    // arrives: the same words, the narrower set.
    expect(
      await screen.findByRole("option", { name: /^Entropy/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: /^Loss/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("combobox", { name: "Filter the terms due for review" }),
    ).toHaveValue("entrop");
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

    fireEvent.click(screen.getByRole("button", { name: "Due for review" }));

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

  it("navigates the due list from the same field the browse list is walked from", async () => {
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

    fireEvent.click(screen.getByRole("button", { name: "Due for review" }));
    await screen.findByRole("option", { name: /^One/ });

    // One focusable widget in both modes: the field owns the cursor and the list
    // is its popup, so the list takes no tab stop of its own.
    const listbox = screen.getByRole("listbox");
    expect(listbox).not.toHaveAttribute("tabindex");
    expect(listbox).not.toHaveAttribute("aria-activedescendant");

    const field = screen.getByRole("combobox");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(field).toHaveAttribute("aria-activedescendant", "glossary-option-1");
    fireEvent.keyDown(field, { key: "Enter" });

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

  it("filters one list from a pair of pressed controls, not a tab strip", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    mount();
    // The `aria-controls` idref is named only once there is a list to name, so
    // the assertions below wait for the first row.
    await screen.findByRole("option", { name: "Alpha" });
    const browse = screen.getByRole("button", { name: "All terms" });
    const study = screen.getByRole("button", { name: "Due for review" });

    // What the controls switch is the content of a listbox, so no tab is
    // announced without the panel that would complete it.
    expect(screen.queryAllByRole("tab")).toHaveLength(0);
    expect(screen.queryByRole("tablist")).toBeNull();
    expect(screen.queryByRole("tabpanel")).toBeNull();

    // Each control names that list, and the pressed one says which filter holds.
    const listbox = screen.getByRole("listbox");
    expect(browse).toHaveAttribute("aria-controls", listbox.id);
    expect(study).toHaveAttribute("aria-controls", listbox.id);
    expect(browse).toHaveAttribute("aria-pressed", "true");
    expect(study).toHaveAttribute("aria-pressed", "false");
    expect(
      screen.getByRole("group", { name: "Glossary listing" }),
    ).toContainElement(study);

    // Both controls are ordinary tab stops: no roving tabindex hides one.
    expect(browse.tabIndex).toBe(0);
    expect(study.tabIndex).toBe(0);

    fireEvent.click(study);

    expect(
      await screen.findByRole("option", { name: /Due term/ }),
    ).toBeInTheDocument();
    expect(study).toHaveAttribute("aria-pressed", "true");
    expect(browse).toHaveAttribute("aria-pressed", "false");
  });

  it("names the list from the field only while there is a list to name", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/search": { terms: [] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    const field = screen.getByRole("combobox");
    expect(field).toHaveAttribute(
      "aria-controls",
      screen.getByRole("listbox").id,
    );
    expect(field).toHaveAttribute("aria-expanded", "true");

    fireEvent.input(field, { target: { value: "nothing" } });
    await screen.findByText("No terms match that search.");

    // The popup is gone, so the idref would resolve to nothing: a combobox
    // naming a listbox that is not rendered announces a relationship the
    // surface is not holding, which is what the mode controls beside it guard.
    expect(field).not.toHaveAttribute("aria-controls");
    expect(field).toHaveAttribute("aria-expanded", "false");
  });

  it("leaves the arrow keys to the term list the controls sit above", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    mount();
    const browse = await screen.findByRole("button", { name: "All terms" });
    const study = screen.getByRole("button", { name: "Due for review" });

    // A pressed-state control is reached by Tab and activated by Space or Enter,
    // so no arrow is spoken for here: all four stay with the term list.
    for (const key of ["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"]) {
      fireEvent.keyDown(browse, { key });
    }

    expect(browse).toHaveAttribute("aria-pressed", "true");
    expect(study).toHaveAttribute("aria-pressed", "false");
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

    fireEvent.click(screen.getByRole("button", { name: "Due for review" }));

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

    fireEvent.click(screen.getByRole("button", { name: "Due for review" }));
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

  it("renders the list the mode it is given names, without a click of its own", async () => {
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
      screen.getByRole("button", { name: "Due for review" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("combobox", { name: "Filter the terms due for review" }),
    ).toBeInTheDocument();
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

    fireEvent.click(screen.getByRole("button", { name: "Due for review" }));
    expect(surface.mode()).toBe("study");

    fireEvent.click(screen.getByRole("button", { name: "All terms" }));
    expect(surface.mode()).toBe("browse");
  });

  it("asks for nothing when the filter pressed is the one already holding", async () => {
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
    const browse = screen.getByRole("button", { name: "All terms" });
    expect(browse).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(browse);

    // The owner mirrors a mode to a pushed history entry, so reporting the mode
    // already holding buys a way back that undoes nothing the reader can see.
    expect(surface.modeAsks).toEqual([]);
    expect(browse).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByRole("button", { name: "Due for review" }));

    expect(surface.modeAsks).toEqual(["study"]);
  });

  it("holds the peeked term when a mode arrives from outside", async () => {
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

    // Both listings hold the term the address names, so the peek is the same
    // definition on either side of the change: the mode moved the list, not the
    // term the reader was reading.
    await screen.findByRole("option", { name: /^One/ });
    expect(screen.getByRole("option", { name: /^Shared/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("option", { name: /^One/ })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("holds focus on the control it is on when the mode arrives from outside", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    const surface = mount({ mode: "study" });
    const browse = await screen.findByRole("button", { name: "All terms" });
    const study = screen.getByRole("button", { name: "Due for review" });
    study.focus();

    surface.showMode("browse");

    // Both controls stay in the tab order, so a mode change moves no tab stop
    // for focus to have to follow.
    expect(document.activeElement).toBe(study);
    expect(browse).toHaveAttribute("aria-pressed", "true");
    expect(study).toHaveAttribute("aria-pressed", "false");
    expect(study.tabIndex).toBe(0);
  });

  it("keeps focus in the field when the mode moves under it", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": { terms: [term("notes/a.org::0", "Alpha")] },
        "/api/glossary/due": { terms: [term("notes/due.org::0", "Due term")] },
        "/api/note/context": contextFor("notes/a.org::0", "Alpha", "Body."),
      }),
    );

    const surface = mount();
    const study = screen.getByRole("button", { name: "Due for review" });
    const field = await screen.findByRole("combobox");
    field.focus();
    expect(document.activeElement).toBe(field);

    surface.showMode("study");
    await screen.findByRole("option", { name: /Due term/ });

    // The field is one element across both modes, so a mode change neither
    // removes it nor drops the focus it was holding.
    expect(document.activeElement).toBe(field);
    expect(study).toHaveAttribute("aria-pressed", "true");

    surface.showMode("browse");
    await screen.findByRole("option", { name: "Alpha" });

    expect(document.activeElement).toBe(field);
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

    fireEvent.click(screen.getByRole("button", { name: "Due for review" }));
    await screen.findByRole("heading", { name: "How terms come due" });

    expect(queryUrl.writes).toEqual(["entropy"]);
    expect(queryUrl.read()).toBe("entropy");

    surface.showMode("browse");
    expect(
      await screen.findByRole("option", { name: "Entropy" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toHaveValue("entropy");
  });

  it("reads the page after the first, holding each term exactly once", async () => {
    vi.stubGlobal("fetch", pagedFetch(twoPageWorld));

    mount();
    await screen.findByRole("option", { name: "Beta" });

    // The cut is stated before it is continued: a list ending at Beta with no
    // word about the rest is a list a reader takes for the whole glossary.
    expect(screen.getByText("2 of 4 terms read.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));

    await screen.findByRole("option", { name: "Delta" });
    expect(screen.getAllByRole("option").map((row) => row.textContent)).toEqual([
      "Alpha",
      "Beta",
      "Gamma",
      "Delta",
    ]);
    // Nothing follows the last page, so nothing is claimed and nothing offered.
    expect(screen.queryByText(/terms read\./)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Read more terms" }),
    ).not.toBeInTheDocument();
  });

  it("continues the list when its own scrollport reaches the end", async () => {
    vi.stubGlobal("fetch", pagedFetch(twoPageWorld));

    mount();
    await screen.findByRole("option", { name: "Beta" });

    // jsdom lays nothing out, so the list measures zero high and is at its end
    // by the same arithmetic a browser scrolled to the bottom satisfies.
    fireEvent.scroll(screen.getByRole("listbox"));

    expect(
      await screen.findByRole("option", { name: "Gamma" }),
    ).toBeInTheDocument();
  });

  it("holds the peeked term and the cursor across a continuation", async () => {
    vi.stubGlobal("fetch", pagedFetch(twoPageWorld));

    mount();
    await screen.findByRole("option", { name: "Beta" });

    const field = screen.getByRole("combobox");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(field).toHaveAttribute("aria-activedescendant", "glossary-option-1");

    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));
    await screen.findByRole("option", { name: "Delta" });

    // Rows arrive after the ones held, and the peek is keyed by term rather than
    // by row, so neither the cursor nor the definition beside it moves.
    expect(field).toHaveAttribute("aria-activedescendant", "glossary-option-1");
    expect(screen.getByRole("option", { name: "Beta" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("keeps the terms already read when a continuation fails", async () => {
    vi.stubGlobal(
      "fetch",
      pagedFetch((url) =>
        url.pathname === "/api/glossary/terms" && url.searchParams.get("after")
          ? new Response(
              JSON.stringify({
                error: { kind: "unavailable", message: "daemon is down" },
              }),
              { status: 503, headers: { "content-type": "application/json" } },
            )
          : twoPageWorld(url),
      ),
    );

    mount();
    await screen.findByRole("option", { name: "Beta" });

    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));

    expect(
      await screen.findByText(
        "More terms could not be read: unavailable: daemon is down",
      ),
    ).toBeInTheDocument();
    expect(screen.getAllByRole("option")).toHaveLength(2);
    // The page it failed on is still the page it would ask for, so the offer
    // stands as the way to try again.
    expect(
      screen.getByRole("button", { name: "Read more terms" }),
    ).toBeInTheDocument();
  });

  it("stops asking for a page that failed until the reader asks again", async () => {
    const fetch = pagedFetch((url) =>
      url.pathname === "/api/glossary/terms" && url.searchParams.get("after")
        ? new Response(
            JSON.stringify({
              error: { kind: "unavailable", message: "daemon is down" },
            }),
            { status: 503, headers: { "content-type": "application/json" } },
          )
        : twoPageWorld(url),
    );
    vi.stubGlobal("fetch", fetch);

    const attempts = (): string[] =>
      asked(fetch).filter((url) => url.includes("after=past-beta"));

    mount();
    await screen.findByRole("option", { name: "Beta" });

    const list = screen.getByRole("listbox");
    fireEvent.scroll(list);
    await screen.findByText(
      "More terms could not be read: unavailable: daemon is down",
    );
    expect(attempts()).toHaveLength(1);

    fireEvent.scroll(list);
    fireEvent.scroll(list);

    // jsdom lays nothing out, so every gesture reads as the end of the box the
    // way a scrolled list does. The request goes out from the handler itself, so
    // a re-issued page is already recorded here.
    expect(attempts()).toHaveLength(1);

    // The offer is the way to try it again, and asking through it asks once.
    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));
    await waitFor(() => expect(attempts()).toHaveLength(2));
  });

  it("drops the pages read past a boundary the first page no longer names", async () => {
    let firstPageReads = 0;
    vi.stubGlobal(
      "fetch",
      pagedFetch((url) => {
        if (url.pathname !== "/api/glossary/terms") {
          const key = url.searchParams.get("key") ?? "";
          return contextFor(key, key, "Body.");
        }
        if (url.searchParams.get("after") === "past-beta") {
          return {
            terms: [
              term("notes/c.org::0", "Gamma"),
              term("notes/d.org::0", "Delta"),
            ],
            total: 4,
            has_more: false,
            next_position: null,
          };
        }
        firstPageReads += 1;
        // A term filed ahead of Alpha between the two reads, so the first page
        // ends one term earlier and hands out a boundary of its own.
        return firstPageReads === 1
          ? {
              terms: [
                term("notes/a.org::0", "Alpha"),
                term("notes/b.org::0", "Beta"),
              ],
              total: 4,
              has_more: true,
              next_position: "past-beta",
            }
          : {
              terms: [
                term("notes/aa.org::0", "Aardvark"),
                term("notes/a.org::0", "Alpha"),
              ],
              total: 5,
              has_more: true,
              next_position: "past-alpha",
            };
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Beta" });
    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));
    await screen.findByRole("option", { name: "Delta" });

    fireEvent(window, new Event("focus"));
    await screen.findByRole("option", { name: "Aardvark" });

    // Gamma and Delta were read from a boundary this first page does not hand
    // out, so they are rows of an order that no longer holds; keeping them would
    // show a listing no request ever answered with.
    await waitFor(() =>
      expect(screen.getAllByRole("option").map((row) => row.textContent)).toEqual(
        ["Aardvark", "Alpha"],
      ),
    );
    expect(screen.getByText("2 of 5 terms read.")).toBeInTheDocument();
  });

  it("keeps the terms read when a re-read of the first page fails", async () => {
    let reads = 0;
    vi.stubGlobal(
      "fetch",
      pagedFetch((url) => {
        if (url.pathname !== "/api/glossary/terms") {
          const key = url.searchParams.get("key") ?? "";
          return contextFor(key, key, "Body.");
        }
        reads += 1;
        return reads === 1
          ? {
              terms: [
                term("notes/a.org::0", "Alpha"),
                term("notes/b.org::0", "Beta"),
              ],
              total: 2,
              has_more: false,
              next_position: null,
            }
          : new Response(
              JSON.stringify({
                error: { kind: "unavailable", message: "daemon is down" },
              }),
              { status: 503, headers: { "content-type": "application/json" } },
            );
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Beta" });

    fireEvent(window, new Event("focus"));
    await screen.findByText("unavailable: daemon is down");

    // A listing on screen is not owed to the request that refreshed it: the
    // failure is stated beside the terms already read rather than in place of
    // them, which would take the whole list away for the duration of an outage.
    expect(screen.getAllByRole("option").map((row) => row.textContent)).toEqual([
      "Alpha",
      "Beta",
    ]);
  });

  it("holds no continuation over the listing the mode switched to", async () => {
    let releaseSecondPage = (): void => {};
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL) => {
        const url = new URL(String(input), "http://slipbox.test");
        if (url.pathname === "/api/glossary/due") {
          return Promise.resolve(
            ok({
              terms: [
                term("notes/one.org::0", "One", { sr_due: "2026-01-01" }),
              ],
              total: 2,
              has_more: true,
              next_position: "past-one",
            }),
          );
        }
        if (url.pathname === "/api/glossary/terms") {
          if (url.searchParams.get("after") === "past-beta") {
            // Left in flight, so the request outlives the listing that asked.
            return new Promise<Response>((resolve) => {
              releaseSecondPage = () =>
                resolve(
                  ok({
                    terms: [term("notes/c.org::0", "Gamma")],
                    total: 3,
                    has_more: false,
                    next_position: null,
                  }),
                );
            });
          }
          return Promise.resolve(
            ok({
              terms: [
                term("notes/a.org::0", "Alpha"),
                term("notes/b.org::0", "Beta"),
              ],
              total: 3,
              has_more: true,
              next_position: "past-beta",
            }),
          );
        }
        const key = url.searchParams.get("key") ?? "";
        return Promise.resolve(ok(contextFor(key, key, "Body.")));
      }) as unknown as typeof fetch,
    );

    const surface = mount();
    await screen.findByRole("option", { name: "Beta" });
    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));
    expect(
      screen.getByRole("button", { name: /Reading more terms/ }),
    ).toBeDisabled();

    surface.showMode("study");
    await screen.findByRole("option", { name: /^One/ });

    // The flight belongs to the listing it was started for, so the listing that
    // replaced it is not reported as mid-continuation and its own way onward is
    // one the reader can take.
    expect(screen.getByRole("button", { name: "Read more terms" })).toBeEnabled();

    releaseSecondPage();
  });

  it("states a search cut without offering a continuation it cannot honour", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [term("notes/a.org::0", "Alpha")],
          total: 1,
          has_more: false,
        },
        // Ranked, so the page carries a total and a cut but no position: two of
        // nine matches, with no token to ask for the seven behind them.
        "/api/glossary/search": {
          terms: [
            term("notes/e.org::0", "Entropy"),
            term("notes/f.org::0", "Entropy pool"),
          ],
          total: 9,
          has_more: true,
        },
        "/api/note/context": contextFor("notes/e.org::0", "Entropy", "Body."),
      }),
    );

    mount();
    await screen.findByRole("option", { name: "Alpha" });

    fireEvent.input(screen.getByRole("combobox"), {
      target: { value: "entrop" },
    });
    await screen.findByRole("option", { name: "Entropy pool" });

    expect(
      screen.getByText(
        "2 of 9 matches shown; a narrower search reaches the rest.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Read more terms" }),
    ).not.toBeInTheDocument();
  });

  it("starts the pages over when the mode takes the listing away", async () => {
    vi.stubGlobal(
      "fetch",
      pagedFetch((url) =>
        url.pathname === "/api/glossary/due"
          ? {
              terms: [term("notes/one.org::0", "One")],
              total: 1,
              has_more: false,
              next_position: null,
            }
          : twoPageWorld(url),
      ),
    );

    const surface = mount();
    await screen.findByRole("option", { name: "Beta" });
    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));
    await screen.findByRole("option", { name: "Delta" });

    surface.showMode("study");
    await screen.findByRole("option", { name: /^One/ });
    expect(screen.getAllByRole("option")).toHaveLength(1);

    surface.showMode("browse");
    await screen.findByRole("option", { name: "Beta" });

    // The pages belong to the listing they were read from, so coming back reads
    // it from its first page rather than restoring rows read under the other.
    expect(screen.getAllByRole("option")).toHaveLength(2);
  });

  it("continues the due list from the position that listing handed out", async () => {
    const fetch = pagedFetch((url) => {
      if (url.pathname === "/api/glossary/due") {
        return url.searchParams.get("after") === "past-two"
          ? {
              terms: [term("notes/three.org::0", "Three")],
              total: 3,
              has_more: false,
              next_position: null,
            }
          : {
              terms: [
                term("notes/one.org::0", "One"),
                term("notes/two.org::0", "Two"),
              ],
              total: 3,
              has_more: true,
              next_position: "past-two",
            };
      }
      const key = url.searchParams.get("key") ?? "";
      return contextFor(key, key, "Body.");
    });
    vi.stubGlobal("fetch", fetch);

    mount({ mode: "study" });
    await screen.findByRole("option", { name: /^Two/ });

    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));

    expect(
      await screen.findByRole("option", { name: /^Three/ }),
    ).toBeInTheDocument();
    // The token is echoed rather than composed: the listing that minted it is
    // the only one that can read it back.
    expect(asked(fetch)).toContain("/api/glossary/due?limit=200&after=past-two");
  });

  it("keeps the pages read when a filter narrows the due list", async () => {
    const fetch = pagedFetch((url) => {
      if (url.pathname === "/api/glossary/due") {
        return url.searchParams.get("after") === "past-prior"
          ? {
              terms: [term("notes/loss.org::0", "Entropic loss")],
              total: 3,
              has_more: false,
              next_position: null,
            }
          : {
              terms: [
                term("notes/entropy.org::0", "Entropy"),
                term("notes/prior.org::0", "Prior"),
              ],
              total: 3,
              has_more: true,
              next_position: "past-prior",
            };
      }
      const key = url.searchParams.get("key") ?? "";
      return contextFor(key, key, "Body.");
    });
    vi.stubGlobal("fetch", fetch);

    mount({ mode: "study" });
    await screen.findByRole("option", { name: /^Prior/ });
    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));
    await screen.findByRole("option", { name: /^Entropic loss/ });

    fireEvent.input(
      screen.getByRole("combobox", { name: "Filter the terms due for review" }),
      { target: { value: "entrop" } },
    );

    await waitFor(() =>
      expect(
        screen.queryByRole("option", { name: /^Prior/ }),
      ).not.toBeInTheDocument(),
    );
    // The filter is over the rows the surface holds, second page included, and
    // asks the index for nothing: narrowing is not a re-read.
    expect(
      screen.getByRole("option", { name: /^Entropic loss/ }),
    ).toBeInTheDocument();
    expect(asked(fetch).filter((url) => url.includes("/api/glossary/due"))).toHaveLength(2);
  });

  it("opens the term the address names, marked in the list", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/a.org::0", "Alpha"),
            term("notes/b.org::0", "Beta"),
          ],
          total: 2,
          has_more: false,
        },
        "/api/note/context": contextFor("notes/b.org::0", "Beta", "Beta body."),
      }),
    );

    mount({ termUrl: memoryTermUrl("notes/b.org::0") });

    const beta = await screen.findByRole("option", { name: "Beta" });
    expect(beta).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("option", { name: "Alpha" })).toHaveAttribute(
      "aria-selected",
      "false",
    );
    expect(screen.getByRole("combobox")).toHaveAttribute(
      "aria-activedescendant",
      "glossary-option-1",
    );
    expect(await screen.findByText("Beta body.")).toBeInTheDocument();
  });

  it("names the term the reader selects in the address", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/a.org::0", "Alpha"),
            term("notes/b.org::0", "Beta"),
          ],
          total: 2,
          has_more: false,
        },
        "/api/note/context": contextFor("notes/b.org::0", "Beta", "Beta body."),
      }),
    );
    const termUrl = memoryTermUrl();

    mount({ termUrl });
    await screen.findByRole("option", { name: "Beta" });

    // The first row is peeked so the pane is never blank, which is a default
    // rather than a choice: only a term the reader reached is worth an address.
    expect(termUrl.writes).toEqual([]);

    fireEvent.mouseEnter(screen.getByRole("option", { name: "Beta" }));

    expect(termUrl.writes).toEqual(["notes/b.org::0"]);
    expect(termUrl.read()).toBe("notes/b.org::0");
  });

  it("names the row the peek falls back to when the list drops the addressed one", async () => {
    vi.stubGlobal(
      "fetch",
      routedFetch({
        "/api/glossary/terms": {
          terms: [
            term("notes/a.org::0", "Alpha"),
            term("notes/b.org::0", "Beta"),
          ],
          total: 2,
          has_more: false,
        },
        "/api/glossary/search": {
          terms: [term("notes/g.org::0", "Gamma")],
          total: 1,
          has_more: false,
        },
        "/api/note/context": contextFor("notes/g.org::0", "Gamma", "Gamma body."),
      }),
    );
    const termUrl = memoryTermUrl();

    mount({ termUrl });
    await screen.findByRole("option", { name: "Beta" });

    fireEvent.mouseEnter(screen.getByRole("option", { name: "Beta" }));
    expect(termUrl.read()).toBe("notes/b.org::0");

    fireEvent.input(screen.getByRole("combobox"), { target: { value: "gamma" } });
    const gamma = await screen.findByRole("option", { name: "Gamma" });

    // The search took the addressed row away and the first row stood in for it.
    // An address left naming the row that went is read as naming the definition
    // beside it, which is a term the reader never asked for.
    expect(gamma).toHaveAttribute("aria-selected", "true");
    expect(termUrl.read()).toBe("notes/g.org::0");
    expect(termUrl.writes).toEqual(["notes/b.org::0", "notes/g.org::0"]);
  });

  it("states an address naming no term and leaves the list usable", async () => {
    vi.stubGlobal(
      "fetch",
      pagedFetch((url) => {
        if (url.pathname === "/api/glossary/term") {
          return new Response(
            JSON.stringify({
              error: {
                kind: "not-found",
                message: "no glossary term found for the given key",
              },
            }),
            { status: 404, headers: { "content-type": "application/json" } },
          );
        }
        if (url.pathname === "/api/glossary/terms") {
          return {
            terms: [
              term("notes/a.org::0", "Alpha"),
              term("notes/b.org::0", "Beta"),
            ],
            total: 2,
            has_more: false,
          };
        }
        return contextFor("notes/a.org::0", "Alpha", "Alpha body.");
      }),
    );

    mount({ termUrl: memoryTermUrl("notes/gone.org::0") });
    await screen.findByRole("option", { name: "Alpha" });

    expect(
      await screen.findByText("No glossary term is named notes/gone.org::0."),
    ).toBeInTheDocument();
    // No row stands in for the term the address named: a first term shown under
    // someone else's address is a definition the reader would take for that one.
    expect(screen.getByRole("option", { name: "Alpha" })).toHaveAttribute(
      "aria-selected",
      "false",
    );
    expect(
      screen.getByText("Select a term to read its definition."),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("option", { name: "Alpha" }));

    expect(await screen.findByText("Alpha body.")).toBeInTheDocument();
    expect(
      screen.queryByText("No glossary term is named notes/gone.org::0."),
    ).not.toBeInTheDocument();
  });

  it("distinguishes an address naming a term the pages read do not hold", async () => {
    vi.stubGlobal(
      "fetch",
      pagedFetch((url) => {
        if (url.pathname === "/api/glossary/term") {
          return { term: term("notes/z.org::0", "Zeta") };
        }
        if (url.pathname === "/api/glossary/terms") {
          return {
            terms: [
              term("notes/a.org::0", "Alpha"),
              term("notes/b.org::0", "Beta"),
            ],
            total: 3,
            has_more: false,
          };
        }
        return contextFor("notes/z.org::0", "Zeta", "Zeta body.");
      }),
    );

    mount({ termUrl: memoryTermUrl("notes/z.org::0") });
    await screen.findByRole("option", { name: "Alpha" });

    // The index holds the term, so it is read and peeked; what the list says is
    // why the reader cannot see it among the rows, which is a different fact
    // from the term not existing.
    expect(
      await screen.findByText("Zeta is not among the terms read so far."),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("heading", { name: "Zeta", level: 2 }),
    ).toBeInTheDocument();
    expect(await screen.findByText("Zeta body.")).toBeInTheDocument();
    expect(
      screen.queryByText(/No glossary term is named/),
    ).not.toBeInTheDocument();
  });

  it("takes up the addressed term when a later page brings it in", async () => {
    vi.stubGlobal(
      "fetch",
      pagedFetch((url) =>
        url.pathname === "/api/glossary/term"
          ? { term: term("notes/d.org::0", "Delta") }
          : twoPageWorld(url),
      ),
    );

    mount({ termUrl: memoryTermUrl("notes/d.org::0") });
    await screen.findByText("Delta is not among the terms read so far.");

    fireEvent.click(screen.getByRole("button", { name: "Read more terms" }));

    const delta = await screen.findByRole("option", { name: "Delta" });
    expect(delta).toHaveAttribute("aria-selected", "true");
    expect(
      screen.queryByText(/is not among the terms read so far\./),
    ).not.toBeInTheDocument();
  });

  it("leaves the addressed term where it is when the mode changes", async () => {
    vi.stubGlobal("fetch", pagedFetch(twoListWorld));
    const termUrl = memoryTermUrl("notes/b.org::0");

    const surface = mount({ termUrl });
    await screen.findByRole("option", { name: "Beta" });

    surface.showMode("study");
    await screen.findByRole("option", { name: /^One/ });

    // A mode change pushes a history entry, and the surface reports the mode
    // before the entry is pushed: clearing the term here strips it from the
    // entry being left, so the way back lands on a glossary naming nothing.
    expect(termUrl.read()).toBe("notes/b.org::0");
    expect(termUrl.writes).toEqual([]);
    // The due listing does not hold it, which is a fact about the listing rather
    // than grounds for standing another term under its address.
    expect(
      await screen.findByText("Beta is not among the terms read so far."),
    ).toBeInTheDocument();
  });

  it("re-seeds the peek from the address when the mode comes back", async () => {
    vi.stubGlobal("fetch", pagedFetch(twoListWorld));
    const termUrl = memoryTermUrl("notes/b.org::0");

    const surface = mount({ termUrl });
    await screen.findByRole("option", { name: "Beta" });

    surface.showMode("study");
    await screen.findByRole("option", { name: /^One/ });
    surface.showMode("browse");

    // The address is where the open term lives, so the listing that comes back
    // reads it again rather than starting from its first row.
    const beta = await screen.findByRole("option", { name: "Beta" });
    expect(beta).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("option", { name: "Alpha" })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });
});
