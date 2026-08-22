import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";

import { visibleText } from "../test/visible-text.js";
import { RelationsFooter } from "./RelationsFooter.jsx";
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

function context(
  forward_links: ForwardLinkRecord[],
  backlinks: BacklinkRecord[],
): NoteContext {
  return {
    note: node("notes/self.org::0", "Self"),
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
    backlinks,
    forward_links,
  };
}

const inertNav: Navigation = { glance: () => {}, pin: () => {}, go: () => {} };

describe("RelationsFooter", () => {
  it("renders the two relation groups from the fetched context", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [forward(node("notes/a.org::0", "Alpha"), "cites Alpha")],
            [backward(node("notes/x.org::0", "Ex"))],
          )}
        />
      </NavigationProvider>
    ));

    expect(screen.getByText("Links to")).toBeInTheDocument();
    expect(screen.getByText("Linked from")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Alpha" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Ex" })).toBeInTheDocument();
    expect(screen.getByText("cites Alpha")).toBeInTheDocument();
  });

  // A relation row is a fragment of a note, so the formula that linked two
  // notes is typeset in it rather than shown as its TeX source.
  it("typesets math in a row preview through KaTeX", () => {
    const { container } = render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context(
            [
              forward(
                node("notes/a.org::0", "Alpha"),
                "bounded by \\(\\sum_n x_n\\) throughout",
              ),
            ],
            [],
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

  it("omits a group with no relations", () => {
    render(() => (
      <NavigationProvider navigation={inertNav}>
        <RelationsFooter
          context={context([forward(node("notes/a.org::0", "Alpha"))], [])}
        />
      </NavigationProvider>
    ));

    expect(screen.getByText("Links to")).toBeInTheDocument();
    expect(screen.queryByText("Linked from")).not.toBeInTheDocument();
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
