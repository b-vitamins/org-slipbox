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
import type { FilingMove } from "./spine-navigation.js";
import { __resetRefocusForTests } from "../data/refetch-on-focus.js";
import { NavigationProvider, type Navigation } from "../org/navigation.jsx";
import type {
  BacklinkRecord,
  BridgeEvidenceRecord,
  ExplorationEntry,
  ForwardLinkRecord,
  NodeRecord,
  NoteContext,
  NotePlace,
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
  forwardTotal?: number | null;
  backwardTotal?: number | null;
  forwardOccurrences?: number;
  backwardOccurrences?: number;
  place?: NotePlace | null;
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
    ...(counts.place === null
      ? {}
      : { place: counts.place ?? { ordinal: 1, total: 1 } }),
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

const inertReadOn: FilingMove = {
  address: (target) => `?note=${target}`,
  open: () => {},
};

function stubReadOn(address = "?note=notes/g.org"): FilingMove & {
  open: ReturnType<typeof vi.fn>;
} {
  return { address: () => address, open: vi.fn() };
}

function relatedGroup(container: HTMLElement): Element {
  return container.querySelectorAll(".relations__group")[1] as Element;
}

function mentionsGroup(container: HTMLElement): Element {
  const groups = container.querySelectorAll(".relations__group");
  return groups[groups.length - 1] as Element;
}

function mount(context: NoteContext): HTMLElement {
  return render(() => (
    <NavigationProvider navigation={inertNav}>
      <RelationsFooter readOn={inertReadOn} context={context} />
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

    expect(container.querySelectorAll(".relations__list")).toHaveLength(1);
    expect(container.querySelectorAll(".relations__row")).toHaveLength(2);
    expect(screen.getByRole("link", { name: "Alpha" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Ex" })).toBeInTheDocument();
  });

  it("declares the directed inventory a list", () => {
    const container = mount(
      context([], [backward(node("notes/x.org::0", "Ex"), "cites Self")]),
    );

    expect(container.querySelector(".relations__list")?.getAttribute("role")).toBe(
      "list",
    );
  });

  it("names each row's direction for a screen reader", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          readOn={inertReadOn}
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
          readOn={inertReadOn}
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
          readOn={inertReadOn}
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

  it("renders a preview no longer than the character bound", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          readOn={inertReadOn}
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
          readOn={inertReadOn}
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

  it("states how many backlinks it holds back when the note has more", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          readOn={inertReadOn}
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

  it("says nothing about a note linked to twice from one note", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          readOn={inertReadOn}
          context={context([], [backward(node("notes/x.org::0", "Ex"))], {
            backwardTotal: 1,
            backwardOccurrences: 2,
          })}
        />
      </NavigationProvider>
    ));

    expect(container.querySelector(".relations__shortfall")).toBeNull();
  });

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
          readOn={inertReadOn}
          context={context(
            [forward(node("notes/a.org::0", "Alpha"))],
            [backward(node("notes/x.org::0", "Ex"))],
          )}
        />
      </NavigationProvider>
    ));

    expect(container.querySelector(".relations__shortfall")).toBeNull();
  });

  it("measures what is shown by the rows it rendered, not by the array", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          readOn={inertReadOn}
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

  it("states a cut in each direction against that direction's own total", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          readOn={inertReadOn}
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

  it("offers a note with no link the mention group alone", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter readOn={inertReadOn} context={context([], [])} />
      </NavigationProvider>
    ));

    expect(container.querySelectorAll(".relations__group")).toHaveLength(1);
    expect(screen.queryByText("Links")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Unlinked mentions" }),
    ).toBeInTheDocument();
  });
});

function filed(place: NotePlace): NoteContext {
  return { ...context([], []), place };
}

const BETWEEN: NotePlace = {
  ordinal: 2,
  total: 3,
  earlier: { node_key: "notes/a.org", title: "Alpha" },
  later: { node_key: "notes/g.org", title: "Gamma" },
};

function mountReadOn(place: NotePlace, readOn: FilingMove): HTMLElement {
  return render(() => (
    <NavigationProvider navigation={inertNav}>
      <RelationsFooter readOn={readOn} context={filed(place)} />
    </NavigationProvider>
  )).container;
}

describe("RelationsFooter reading on", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("offers a move to each side, naming the side and the note it reaches", () => {
    const container = mountReadOn(BETWEEN, inertReadOn);

    const links = [...container.querySelectorAll(".read-on__link")];
    expect(
      links.map((link) => [
        link.querySelector(".read-on__side")?.textContent,
        link.querySelector(".read-on__title")?.textContent,
      ]),
    ).toEqual([
      ["Filed before this", "Alpha"],
      ["Filed after this", "Gamma"],
    ]);
    expect(screen.getByRole("link", { name: /Alpha/ })).toHaveAccessibleName(
      /Filed before this/,
    );
  });

  it("stands above the relations footer rather than inside it", () => {
    const container = mountReadOn(BETWEEN, inertReadOn);

    const pair = container.querySelector(".read-on") as HTMLElement;
    const footer = container.querySelector(".relations") as HTMLElement;
    expect(
      pair.compareDocumentPosition(footer) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(footer.contains(pair)).toBe(false);
    expect(footer.querySelectorAll("a")).toHaveLength(0);
    expect(container.querySelectorAll(".relations__row")).toHaveLength(0);
  });

  it("offers the one move an end of the order holds, and no gap for the other", () => {
    const container = mountReadOn(
      { ordinal: 1, total: 3, later: BETWEEN.later },
      inertReadOn,
    );

    expect(container.querySelectorAll(".read-on__link")).toHaveLength(1);
    expect(
      screen.getByRole("link", { name: /^Filed after this/ }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Filed before this")).not.toBeInTheDocument();
  });

  it("states no move where the payload states no position", () => {
    const container = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter readOn={inertReadOn} context={context([], [])} />
      </NavigationProvider>
    )).container;

    expect(container.querySelector(".read-on")).toBeNull();
  });

  it("states no move where the payload carries no place", () => {
    const container = mount(context([], [], { place: null }));

    expect(container.querySelector(".read-on")).toBeNull();
    expect(container.querySelector(".relations")).not.toBeNull();
  });

  it("reads on to the neighbor it names, at the address it carries", () => {
    const readOn = stubReadOn("?note=notes/g.org");
    mountReadOn(BETWEEN, readOn);

    const link = screen.getByRole("link", { name: /^Filed after this/ });
    expect(link).toHaveAttribute("href", "?note=notes/g.org");

    fireEvent.click(link, { detail: 0 });
    expect(readOn.open).toHaveBeenCalledWith("notes/g.org");
  });

  it("leaves a browser gesture to the browser", () => {
    const readOn = stubReadOn("#filed");
    mountReadOn(BETWEEN, readOn);

    const kept = fireEvent.click(
      screen.getByRole("link", { name: /^Filed after this/ }),
      { metaKey: true },
    );

    expect(kept).toBe(true);
    expect(readOn.open).not.toHaveBeenCalled();
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

function ranking(count: number): ExplorationEntry[] {
  return Array.from({ length: count }, (_, i) =>
    bridge(node(`notes/b${i}.org::0`, `Bridged ${i + 1}`), [
      via("notes/c1.org::0", "Measure"),
    ]),
  );
}

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
    expect(screen.getByRole("link", { name: "Alpha" })).toBeInTheDocument();
  });
});

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

function scanned(count: number): UnlinkedReferenceRecord[] {
  return Array.from({ length: count }, (_, i) =>
    mention(node(`notes/m${i}.org::0`, `Naming ${i + 1}`), "Self is named here"),
  );
}

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
