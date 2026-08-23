import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";

import { visibleText } from "../test/visible-text.js";
import { RelationsFooter } from "./RelationsFooter.jsx";
import { RELATION_PREVIEW_CHARS } from "./relations.js";
import { NavigationProvider, type Navigation } from "../org/navigation.jsx";
import type {
  BacklinkRecord,
  ForwardLinkRecord,
  NodeRecord,
  NoteContext,
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

describe("RelationsFooter", () => {
  it("lists every related note once, in one group", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [forward(node("notes/a.org::0", "Alpha"), "cites Alpha")],
            [
              backward(node("notes/a.org::0", "Alpha"), "cites Self"),
              backward(node("notes/x.org::0", "Ex"), "also cites Self"),
            ],
          )}
        />
      </NavigationProvider>
    ));

    expect(container.querySelectorAll(".relations__group")).toHaveLength(1);
    expect(container.querySelectorAll(".relations__row")).toHaveLength(2);
    expect(screen.getByRole("link", { name: "Alpha" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Ex" })).toBeInTheDocument();
  });

  // The listing drops its markers for the surface's rhythm, and an engine that
  // reads that as a listing meant to be read as prose drops the list semantics
  // with them. Neither engine under test is one of those, so what is asserted is
  // the declaration itself.
  it("declares the directed inventory a list", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context([], [backward(node("notes/x.org::0", "Ex"), "cites Self")])}
        />
      </NavigationProvider>
    ));

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
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [forward(node("notes/a.org::0", "Alpha"))],
            [backward(node("notes/x.org::0", "Ex"))],
            { forwardTotal: null, backwardTotal: null },
          )}
        />
      </NavigationProvider>
    ));

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

  // The footer is a hairline rule plus whatever it lists. A note nothing links
  // to and that links to nothing must not end in a rule with nothing under it.
  it("draws nothing at all for a note with no relations", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter context={context([], [])} />
      </NavigationProvider>
    ));

    expect(container.querySelector(".relations")).toBeNull();
  });
});
