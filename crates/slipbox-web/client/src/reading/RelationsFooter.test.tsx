import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { visibleText } from "../test/visible-text.js";
import {
  RelationsFooter,
  MENTIONS_SCAN_LIMIT,
  MENTIONS_SHOWN,
  RELATED_LENS_LIMIT,
  RELATED_SHOWN,
} from "./RelationsFooter.jsx";
import { RELATION_PREVIEW_CHARS } from "./relations.js";
import { __resetRefocusForTests } from "../data/refetch-on-focus.js";
import { NavigationProvider, type Navigation } from "../org/navigation.jsx";
import type {
  BacklinkRecord,
  BridgeEvidenceRecord,
  ExplorationEntry,
  ForwardLinkRecord,
  NodeRecord,
  NoteContext,
  UnlinkedReferenceRecord,
} from "../api/types.js";

function node(
  key: string,
  title: string,
  explicitId: string | null = null,
): NodeRecord {
  return {
    node_key: key,
    explicit_id: explicitId,
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

function forward(dest: NodeRecord, preview = ""): ForwardLinkRecord {
  return {
    destination_note: dest,
    row: 1,
    col: 1,
    preview,
    explanation: { kind: "forward-link" },
  };
}

function backward(source: NodeRecord, preview = ""): BacklinkRecord {
  return {
    source_note: source,
    source_anchor: null,
    row: 1,
    col: 1,
    preview,
    explanation: { kind: "backlink" },
  };
}

interface ContextCounts {
  /**
   * Notes per direction, as the payload totals them. `null` for a daemon older
   * than the fields, which answers without them.
   */
  forwardTotal?: number | null;
  backwardTotal?: number | null;
  /** The note record's link rows, which count occurrences and not notes. */
  forwardOccurrences?: number;
  backwardOccurrences?: number;
}

function context(
  forward_links: ForwardLinkRecord[],
  backlinks: BacklinkRecord[],
  counts: ContextCounts = {},
): NoteContext {
  return {
    note: {
      ...node("notes/self.org::0", "Self"),
      forward_link_count: counts.forwardOccurrences ?? forward_links.length,
      backlink_count: counts.backwardOccurrences ?? backlinks.length,
    },
    source: {
      file_path: "notes/self.org",
      start_line: 1,
      line_count: 1,
      total_lines: 1,
      content: "",
      truncated_before: false,
      truncated_after: false,
    },
    node_start_line: 1,
    node_line_count: 1,
    place: { ordinal: 1, total: 1 },
    backlinks,
    forward_links,
    ...(counts.backwardTotal === null
      ? {}
      : { backlink_note_total: counts.backwardTotal ?? backlinks.length }),
    ...(counts.forwardTotal === null
      ? {}
      : { forward_link_note_total: counts.forwardTotal ?? forward_links.length }),
  };
}

const inertNav: Navigation = { glance: () => {}, pin: () => {}, go: () => {} };

/** The ranked group, which stands beside the directed inventory. */
function relatedGroup(container: HTMLElement): Element {
  return container.querySelectorAll(".relations__group")[1] as Element;
}

/** The mention group, which stands last whether the others stand at all. */
function mentionsGroup(container: HTMLElement): Element {
  const groups = container.querySelectorAll(".relations__group");
  return groups[groups.length - 1] as Element;
}

function mount(context: NoteContext): HTMLElement {
  return render(() => (
    <NavigationProvider navigation={inertNav}>
      <RelationsFooter context={context} />
    </NavigationProvider>
  )).container;
}

describe("RelationsFooter", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("lists every related note once, in one directed group", () => {
    const container = mount(
      context(
        [forward(node("notes/a.org::0", "Alpha"), "cites Alpha")],
        [
          backward(node("notes/a.org::0", "Alpha"), "cites Self"),
          backward(node("notes/x.org::0", "Ex"), "also cites Self"),
        ],
      ),
    );

    // Both directions in one listing, with the deferred group closed beside it.
    expect(container.querySelectorAll(".relations__list")).toHaveLength(1);
    expect(container.querySelectorAll(".relations__row")).toHaveLength(2);
    expect(screen.getByRole("link", { name: "Alpha" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Ex" })).toBeInTheDocument();
  });

  // The listing drops its markers for the surface's rhythm, and an engine that
  // reads that as a listing meant to be read as prose drops the list semantics
  // with them. Neither engine under test is one of those, so what is asserted is
  // the declaration itself.
  it("declares the directed inventory a list", () => {
    const container = mount(
      context([], [backward(node("notes/x.org::0", "Ex"), "cites Self")]),
    );

    expect(container.querySelector(".relations__list")?.getAttribute("role")).toBe(
      "list",
    );
  });

  // A glyph is not a label: a reader who cannot see the mark still has to be
  // told which way the links run.
  it("names each row's direction for a screen reader", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [
              forward(node("notes/a.org::0", "Alpha")),
              forward(node("notes/b.org::0", "Beta")),
            ],
            [
              backward(node("notes/a.org::0", "Alpha"), "cites Self"),
              backward(node("notes/x.org::0", "Ex"), "also cites Self"),
            ],
          )}
        />
      </NavigationProvider>
    ));

    expect(
      screen.getAllByRole("img").map((mark) => mark.getAttribute("aria-label")),
    ).toEqual(["Links to and from", "Links to", "Linked from"]);
  });

  it("shows no preview on a row the note only links out to", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [forward(node("notes/a.org::0", "Alpha"), "cites Alpha")],
            [],
          )}
        />
      </NavigationProvider>
    ));

    expect(screen.queryByText("cites Alpha")).not.toBeInTheDocument();
    expect(container.querySelector(".relations__preview")).toBeNull();
  });

  // A relation row is a fragment of a note, so the formula that linked two
  // notes is typeset in it rather than shown as its TeX source.
  it("typesets math in a row preview through KaTeX", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [],
            [
              backward(
                node("notes/x.org::0", "Ex"),
                "bounded by \\(\\sum_n x_n\\) throughout",
              ),
            ],
          )}
        />
      </NavigationProvider>
    ));

    const preview = container.querySelector(".relations__preview");
    expect(preview?.querySelector(".katex")).not.toBeNull();
    // KaTeX keeps the TeX in its MathML annotation, which is how the math
    // reaches assistive tech; what the reader sees is the typeset glyphs.
    expect(visibleText(preview)).toBe("bounded by ∑n​xn​ throughout");
  });

  // What the row paints is one clipped line, but the text node behind it is
  // what a screen reader reads out in full.
  it("renders a preview no longer than the character bound", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [],
            [
              backward(
                node("notes/x.org::0", "Ex"),
                "quantum ".repeat(RELATION_PREVIEW_CHARS),
              ),
            ],
          )}
        />
      </NavigationProvider>
    ));

    const preview = container.querySelector(".relations__preview");
    expect(preview?.textContent?.length).toBeLessThanOrEqual(
      RELATION_PREVIEW_CHARS + 1,
    );
  });

  it("pins the related note when its row is clicked, via an id target", () => {
    const navigation: Navigation = {
      glance: vi.fn(),
      pin: vi.fn(),
      go: vi.fn(),
    };
    render(() => (
      <NavigationProvider navigation={navigation}>
        <RelationsFooter
          context={context(
            [forward(node("notes/a.org::0", "Alpha", "uuid-1"))],
            [],
          )}
        />
      </NavigationProvider>
    ));

    const link = screen.getByRole("link", { name: "Alpha" });
    // The href is the real URL the router reads, not a decorative hash.
    expect(link.getAttribute("href")).toBe("?note=id%3Auuid-1");
    fireEvent.click(link);

    expect(navigation.pin).toHaveBeenCalledWith({
      id: "uuid-1",
      target: "id:uuid-1",
    });
  });

  // The request bounds each direction, so a full-looking group can still be cut;
  // only the payload's own total says how much of it the footer never received.
  it("states how many backlinks it holds back when the note has more", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context([], [backward(node("notes/x.org::0", "Ex"))], {
            backwardTotal: 42,
          })}
        />
      </NavigationProvider>
    ));

    expect(
      screen.getByText("Showing 1 of 42 notes linking here."),
    ).toBeInTheDocument();
  });

  // The note record counts link rows, and one note may link here twice; the
  // listing holds notes, so trusting that count would state a cut that never was.
  it("says nothing about a note linked to twice from one note", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context([], [backward(node("notes/x.org::0", "Ex"))], {
            backwardTotal: 1,
            backwardOccurrences: 2,
          })}
        />
      </NavigationProvider>
    ));

    expect(container.querySelector(".relations__shortfall")).toBeNull();
  });

  // A daemon older than the totals answers without them. An unknown total is
  // nothing to compare the rows against, so the group states no cut rather than
  // reading the absence as a total of zero.
  it("says nothing about a direction the payload totals not at all", () => {
    const container = mount(
      context(
        [forward(node("notes/a.org::0", "Alpha"))],
        [backward(node("notes/x.org::0", "Ex"))],
        { forwardTotal: null, backwardTotal: null },
      ),
    );

    expect(container.querySelectorAll(".relations__row")).toHaveLength(2);
    expect(container.querySelector(".relations__shortfall")).toBeNull();
  });

  it("says nothing about a group whose links all fit", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [forward(node("notes/a.org::0", "Alpha"))],
            [backward(node("notes/x.org::0", "Ex"))],
          )}
        />
      </NavigationProvider>
    ));

    expect(container.querySelector(".relations__shortfall")).toBeNull();
  });

  // Two records for one note collapse into one row, so what is shown is the rows
  // rendered rather than the array they came from.
  it("measures what is shown by the rows it rendered, not by the array", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [
              forward(node("notes/a.org::0", "Alpha"), "first mention"),
              forward(node("notes/a.org::0", "Alpha"), "second mention"),
            ],
            [],
            { forwardTotal: 5 },
          )}
        />
      </NavigationProvider>
    ));

    expect(
      screen.getByText("Showing 1 of 5 notes linked to."),
    ).toBeInTheDocument();
  });

  // One listing holds both directions, and the request bounds each of them, so a
  // cut in one direction must not be reported as a cut in the other.
  it("states a cut in each direction against that direction's own total", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [forward(node("notes/a.org::0", "Alpha"))],
            [backward(node("notes/x.org::0", "Ex"), "cites Self")],
            { forwardTotal: 3, backwardTotal: 9 },
          )}
        />
      </NavigationProvider>
    ));

    expect(
      screen.getByText("Showing 1 of 3 notes linked to."),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Showing 1 of 9 notes linking here."),
    ).toBeInTheDocument();
  });

  // A note nothing links to and that links to nothing has no inventory and no
  // bridge candidate, both being link topology. What is left is the prose scan,
  // which is the only relation that reaches such a note at all.
  it("offers a note with no link the mention group alone", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter context={context([], [])} />
      </NavigationProvider>
    ));

    expect(container.querySelectorAll(".relations__group")).toHaveLength(1);
    expect(screen.queryByText("Links")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Unlinked mentions" }),
    ).toBeInTheDocument();
  });
});

function via(key: string, title: string): BridgeEvidenceRecord {
  return { node_key: key, explicit_id: null, title };
}

function bridge(
  candidate: NodeRecord,
  viaNotes: BridgeEvidenceRecord[],
): ExplorationEntry {
  return {
    kind: "anchor",
    anchor: candidate,
    explanation: { kind: "bridge-candidate", references: [], via_notes: viaNotes },
  };
}

/** `count` candidates, each reached through one shared connector. */
function ranking(count: number): ExplorationEntry[] {
  return Array.from({ length: count }, (_, i) =>
    bridge(node(`notes/b${i}.org::0`, `Bridged ${i + 1}`), [
      via("notes/c1.org::0", "Measure"),
    ]),
  );
}

/**
 * A fetch double answering `/api/explore` with `entries`, or with an error
 * envelope at any other status. Every URL asked for is recorded, which is what
 * a deferred group's cost is read off.
 */
function stubExplore(entries: ExplorationEntry[], status = 200): string[] {
  const urls: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn((input: RequestInfo | URL) => {
      urls.push(String(input));
      const body =
        status === 200
          ? { lens: "bridges", sections: [{ kind: "bridge-candidates", entries }] }
          : { error: { kind: "internal", message: "the index is locked" } };
      return Promise.resolve(
        new Response(JSON.stringify(body), {
          status,
          headers: { "content-type": "application/json" },
        }),
      );
    }),
  );
  return urls;
}

function explored(urls: string[]): URL[] {
  return urls
    .map((url) => new URL(url, "http://slipbox.test"))
    .filter((url) => url.pathname === "/api/explore");
}

const RELATED = "Related notes";

/** A note with one link, so the footer renders and the group stands beside it. */
function linkedNote(): NoteContext {
  return context([forward(node("notes/a.org::0", "Alpha"))], []);
}

describe("RelationsFooter related notes", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  // A column costs one request while the footer is only read; the lens is asked
  // its question only when a reader asks for it.
  it("requests nothing for the group until it is opened", async () => {
    const urls = stubExplore(ranking(1));
    mount(linkedNote());

    expect(explored(urls)).toHaveLength(0);
    expect(
      screen.getByRole("button", { name: RELATED, expanded: false }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: RELATED }));
    expect(await screen.findByRole("link", { name: "Bridged 1" })).toBeInTheDocument();

    const asked = explored(urls);
    expect(asked).toHaveLength(1);
    expect(asked[0]!.searchParams.get("key")).toBe("notes/self.org::0");
    expect(asked[0]!.searchParams.get("lens")).toBe("bridges");
    expect(asked[0]!.searchParams.get("limit")).toBe(String(RELATED_LENS_LIMIT));
  });

  // The reason the directed inventory declares its role, on the listing beside it.
  it("declares the ranked group's listing a list", async () => {
    stubExplore(ranking(1));
    const container = mount(linkedNote());

    fireEvent.click(screen.getByRole("button", { name: RELATED }));
    expect(
      await screen.findByRole("link", { name: "Bridged 1" }),
    ).toBeInTheDocument();

    expect(
      relatedGroup(container)
        .querySelector(".relations__list")
        ?.getAttribute("role"),
    ).toBe("list");
  });

  it("names a connector once over the rows it reached", async () => {
    const container = mount(linkedNote());
    stubExplore([
      bridge(node("notes/b1.org::0", "First"), [via("notes/c1.org::0", "Measure")]),
      bridge(node("notes/b2.org::0", "Second"), [via("notes/c1.org::0", "Measure")]),
    ]);

    fireEvent.click(screen.getByRole("button", { name: RELATED }));
    expect(await screen.findByRole("link", { name: "First" })).toBeInTheDocument();

    const connectors = relatedGroup(container).querySelectorAll(
      ".relations__connector",
    );
    expect([...connectors].map((label) => label.textContent)).toEqual([
      "via Measure",
    ]);
    expect(screen.getByRole("link", { name: "Second" })).toBeInTheDocument();
  });

  // The ranking is the lens's, and how far a row stands from the connector above
  // it is what put it where it is.
  it("states how many notes reached a row, above one", async () => {
    stubExplore([
      bridge(node("notes/b1.org::0", "Wide"), [
        via("notes/c1.org::0", "Measure"),
        via("notes/c2.org::0", "Entropy"),
        via("notes/c3.org::0", "Prior"),
      ]),
      bridge(node("notes/b2.org::0", "Narrow"), [via("notes/c1.org::0", "Measure")]),
    ]);
    mount(linkedNote());

    fireEvent.click(screen.getByRole("button", { name: RELATED }));
    expect(await screen.findByRole("link", { name: "Wide" })).toBeInTheDocument();

    expect(screen.getByText("through 3 notes")).toBeInTheDocument();
    expect(screen.queryByText("through 1 notes")).not.toBeInTheDocument();
  });

  it("shows a bounded head and offers the rest by count", async () => {
    const container = mount(linkedNote());
    stubExplore(ranking(RELATED_SHOWN + 2));

    fireEvent.click(screen.getByRole("button", { name: RELATED }));
    expect(await screen.findByRole("link", { name: "Bridged 1" })).toBeInTheDocument();
    expect(relatedGroup(container).querySelectorAll(".relations__row")).toHaveLength(
      RELATED_SHOWN,
    );

    fireEvent.click(screen.getByRole("button", { name: "Show 2 more" }));
    expect(relatedGroup(container).querySelectorAll(".relations__row")).toHaveLength(
      RELATED_SHOWN + 2,
    );
    expect(screen.queryByRole("button", { name: /^Show \d+ more$/ })).toBeNull();
  });

  // Two bounds cut the answer, and the rows on screen are evidence of neither:
  // the head says what it holds, and the lens's own limit is a separate sentence.
  it("distinguishes the head's cut from the lens's own bound", async () => {
    stubExplore(ranking(RELATED_LENS_LIMIT));
    mount(linkedNote());

    fireEvent.click(screen.getByRole("button", { name: RELATED }));
    expect(await screen.findByRole("link", { name: "Bridged 1" })).toBeInTheDocument();

    expect(
      screen.getByRole("button", {
        name: `Show ${RELATED_LENS_LIMIT - RELATED_SHOWN} more`,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        `The lens ranks at most ${RELATED_LENS_LIMIT} notes, so the slipbox may hold more.`,
      ),
    ).toBeInTheDocument();
  });

  it("says nothing of a bound the lens did not reach", async () => {
    stubExplore(ranking(2));
    mount(linkedNote());

    fireEvent.click(screen.getByRole("button", { name: RELATED }));
    expect(await screen.findByRole("link", { name: "Bridged 1" })).toBeInTheDocument();

    expect(screen.queryByText(/The lens ranks at most/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Show \d+ more$/ })).toBeNull();
  });

  it("says there are none rather than opening an empty group", async () => {
    const container = mount(linkedNote());
    stubExplore([]);

    fireEvent.click(screen.getByRole("button", { name: RELATED }));
    expect(
      await screen.findByText("No note stands beside this one unlinked."),
    ).toBeInTheDocument();
    expect(relatedGroup(container).querySelector(".relations__connector")).toBeNull();
  });

  // The lens excludes the note's own neighbors, but a row printed twice is the
  // footer's own doing, so the rows it holds are what the exclusion is read off.
  it("keeps a note the directed inventory lists out of the group", async () => {
    const container = mount(linkedNote());
    stubExplore([
      bridge(node("notes/a.org::0", "Alpha"), [via("notes/c1.org::0", "Measure")]),
      bridge(node("notes/b1.org::0", "Bridged 1"), [via("notes/c1.org::0", "Measure")]),
    ]);

    fireEvent.click(screen.getByRole("button", { name: RELATED }));
    expect(await screen.findByRole("link", { name: "Bridged 1" })).toBeInTheDocument();

    const titles = [
      ...relatedGroup(container).querySelectorAll(".relations__link"),
    ].map((link) => link.textContent);
    expect(titles).toEqual(["Bridged 1"]);
  });

  // A failure states itself where the control that asked stands, and asking
  // again is that control, so the group goes back to being closed.
  it("leaves the group closed and states a failed read", async () => {
    const container = mount(linkedNote());
    stubExplore([], 500);

    fireEvent.click(screen.getByRole("button", { name: RELATED }));

    expect(await screen.findByText("the index is locked")).toBeInTheDocument();
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: RELATED, expanded: false }),
      ).toBeInTheDocument();
    });
    expect(relatedGroup(container).querySelector(".relations__list")).toBeNull();
    // The rest of the footer is unaffected by a group that could not be read.
    expect(screen.getByRole("link", { name: "Alpha" })).toBeInTheDocument();
  });
});

/**
 * One occurrence, matched where the text first stands in the line. The line sits
 * in `source` itself unless `under` names a node inside it.
 */
function mention(
  source: NodeRecord,
  preview: string,
  matched = "Self",
  under: NodeRecord = source,
): UnlinkedReferenceRecord {
  return {
    source_note: source,
    source_anchor: under,
    row: 1,
    col: preview.indexOf(matched) + 1,
    preview,
    matched_text: matched,
    explanation: { kind: "unlinked-reference", matched_text: matched },
  };
}

/** `count` mentions, each in a note of its own. */
function scanned(count: number): UnlinkedReferenceRecord[] {
  return Array.from({ length: count }, (_, i) =>
    mention(node(`notes/m${i}.org::0`, `Naming ${i + 1}`), "Self is named here"),
  );
}

/**
 * A fetch double answering the mention scan with `records`, or with an error
 * envelope at any other status. Every URL asked for is recorded.
 */
function stubMentions(
  records: UnlinkedReferenceRecord[],
  status = 200,
): string[] {
  const urls: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn((input: RequestInfo | URL) => {
      urls.push(String(input));
      const body =
        status === 200
          ? { unlinked_references: records }
          : { error: { kind: "internal", message: "the index is locked" } };
      return Promise.resolve(
        new Response(JSON.stringify(body), {
          status,
          headers: { "content-type": "application/json" },
        }),
      );
    }),
  );
  return urls;
}

function asked(urls: string[]): URL[] {
  return urls
    .map((url) => new URL(url, "http://slipbox.test"))
    .filter((url) => url.pathname === "/api/unlinked-references");
}

const MENTIONS = "Unlinked mentions";

describe("RelationsFooter unlinked mentions", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  // The scan walks files on disk, which is the most expensive read the footer
  // can make, so nothing is asked for until a reader asks.
  it("requests nothing for the group until it is opened", async () => {
    const urls = stubMentions(scanned(1));
    mount(linkedNote());

    expect(asked(urls)).toHaveLength(0);
    expect(
      screen.getByRole("button", { name: MENTIONS, expanded: false }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));
    expect(await screen.findByRole("link", { name: "Naming 1" })).toBeInTheDocument();

    const requests = asked(urls);
    expect(requests).toHaveLength(1);
    expect(requests[0]!.searchParams.get("key")).toBe("notes/self.org::0");
    expect(requests[0]!.searchParams.get("limit")).toBe(
      String(MENTIONS_SCAN_LIMIT),
    );
  });

  // The reason the listings above declare their role, on the last of them.
  it("declares the mention group's listing a list", async () => {
    stubMentions(scanned(1));
    const container = mount(linkedNote());

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));
    expect(
      await screen.findByRole("link", { name: "Naming 1" }),
    ).toBeInTheDocument();

    expect(
      mentionsGroup(container)
        .querySelector(".relations__list")
        ?.getAttribute("role"),
    ).toBe("list");
  });

  // The title says which note names this one; the mark says which words do.
  it("marks the matched text inside the line rather than beside it", async () => {
    const container = mount(linkedNote());
    stubMentions([
      mention(node("notes/m1.org::0", "Naming 1"), "compared with Self throughout"),
    ]);

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));
    expect(await screen.findByRole("link", { name: "Naming 1" })).toBeInTheDocument();

    const preview = mentionsGroup(container).querySelector(".relations__preview");
    expect(preview?.textContent).toBe("compared with Self throughout");
    expect(preview?.querySelector("mark.relations__match")?.textContent).toBe("Self");
  });

  it("shows a bounded head and offers the rest by count", async () => {
    const container = mount(linkedNote());
    stubMentions(scanned(MENTIONS_SHOWN + 3));

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));
    expect(await screen.findByRole("link", { name: "Naming 1" })).toBeInTheDocument();
    expect(mentionsGroup(container).querySelectorAll(".relations__row")).toHaveLength(
      MENTIONS_SHOWN,
    );

    fireEvent.click(screen.getByRole("button", { name: "Show 3 more" }));
    expect(mentionsGroup(container).querySelectorAll(".relations__row")).toHaveLength(
      MENTIONS_SHOWN + 3,
    );
    expect(screen.queryByRole("button", { name: /^Show \d+ more$/ })).toBeNull();
  });

  // The head holds back notes and the scan's limit counts occurrences, so a row
  // count is evidence of neither bound and each says what it cut.
  it("distinguishes the head's cut from the scan's own bound", async () => {
    stubMentions(scanned(MENTIONS_SCAN_LIMIT));
    mount(linkedNote());

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));
    expect(await screen.findByRole("link", { name: "Naming 1" })).toBeInTheDocument();

    expect(
      screen.getByRole("button", {
        name: `Show ${MENTIONS_SCAN_LIMIT - MENTIONS_SHOWN} more`,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        `The scan stops at ${MENTIONS_SCAN_LIMIT} mentions, so the slipbox may hold more.`,
      ),
    ).toBeInTheDocument();
  });

  it("says nothing of a bound the scan did not reach", async () => {
    stubMentions(scanned(2));
    mount(linkedNote());

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));
    expect(await screen.findByRole("link", { name: "Naming 1" })).toBeInTheDocument();

    expect(screen.queryByText(/The scan stops at/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Show \d+ more$/ })).toBeNull();
  });

  it("says there are none rather than opening an empty group", async () => {
    const container = mount(linkedNote());
    stubMentions([]);

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));
    expect(
      await screen.findByText("No note names this one without linking to it."),
    ).toBeInTheDocument();
    expect(mentionsGroup(container).querySelector(".relations__list")).toBeNull();
  });

  // The scan excludes an occurrence a link already covers, one occurrence at a
  // time, so a note that links here and names it elsewhere still arrives.
  it("keeps a note the directed inventory lists out of the group", async () => {
    const container = mount(linkedNote());
    stubMentions([
      mention(node("notes/a.org::0", "Alpha"), "Self is named here"),
      mention(node("notes/m1.org::0", "Naming 1"), "Self is named here too"),
    ]);

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));
    expect(await screen.findByRole("link", { name: "Naming 1" })).toBeInTheDocument();

    const titles = [
      ...mentionsGroup(container).querySelectorAll(".relations__link"),
    ].map((link) => link.textContent);
    expect(titles).toEqual(["Naming 1"]);
  });

  // The scan reports the node the line sits in, down to a heading holding no id,
  // while the inventory lists notes: a mention inside a note listed there is the
  // same note twice over, whichever of its headings the line stands under.
  it("keeps a listed note out though a heading in it holds the mention", async () => {
    const container = mount(linkedNote());
    stubMentions([
      mention(
        node("notes/a.org::0", "Alpha"),
        "Self is named here",
        "Self",
        node("notes/a.org::6", "A section of Alpha"),
      ),
      mention(node("notes/m1.org::0", "Naming 1"), "Self is named here too"),
    ]);

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));
    expect(await screen.findByRole("link", { name: "Naming 1" })).toBeInTheDocument();

    const titles = [
      ...mentionsGroup(container).querySelectorAll(".relations__link"),
    ].map((link) => link.textContent);
    expect(titles).toEqual(["Naming 1"]);
  });

  it("leaves the group closed and states a failed read", async () => {
    const container = mount(linkedNote());
    stubMentions([], 500);

    fireEvent.click(screen.getByRole("button", { name: MENTIONS }));

    expect(await screen.findByText("the index is locked")).toBeInTheDocument();
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: MENTIONS, expanded: false }),
      ).toBeInTheDocument();
    });
    expect(mentionsGroup(container).querySelector(".relations__list")).toBeNull();
    expect(screen.getByRole("link", { name: "Alpha" })).toBeInTheDocument();
  });
});
