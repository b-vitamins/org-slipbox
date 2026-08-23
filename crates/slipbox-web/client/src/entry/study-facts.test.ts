import { describe, expect, it } from "vitest";

import { dueStanding, studyFacts } from "./study-facts.js";
import type { NodeRecord } from "../api/types.js";

/** A node record with only the SM-2 drawer fields that matter here set. */
function term(sr: Partial<NodeRecord>): NodeRecord {
  return {
    node_key: "notes/g.org::0",
    explicit_id: null,
    file_path: "notes/g.org",
    title: "A term",
    outline_path: "A term",
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
    ...sr,
  };
}

describe("studyFacts", () => {
  it("says a term has never been reviewed rather than saying nothing", () => {
    expect(studyFacts(term({}))).toEqual([
      { label: "Schedule", value: "Never reviewed" },
    ]);
  });

  it("lists the review schedule in display order", () => {
    const facts = studyFacts(
      term({
        sr_due: "2026-08-01",
        sr_interval: "21",
        sr_reps: "4",
        sr_ease: "2.5",
        sr_last: "2026-07-11",
      }),
    );
    expect(facts).toEqual([
      { label: "Due", value: "2026-08-01" },
      { label: "Interval", value: "21 days" },
      { label: "Reviews", value: "4" },
      { label: "Ease", value: "2.5" },
      { label: "Last reviewed", value: "2026-07-11" },
    ]);
  });

  it("pluralizes a one-day interval as a singular", () => {
    expect(studyFacts(term({ sr_interval: "1" }))).toEqual([
      { label: "Interval", value: "1 day" },
    ]);
  });

  it("omits fields the term does not carry", () => {
    expect(studyFacts(term({ sr_reps: "2" }))).toEqual([
      { label: "Reviews", value: "2" },
    ]);
  });
});

describe("dueStanding", () => {
  it("names an empty drawer as a term never reviewed", () => {
    expect(dueStanding(term({}))).toBe("never reviewed");
  });

  it("dates a term the schedule has already brought round", () => {
    expect(
      dueStanding(term({ sr_due: "2026-07-20", sr_reps: "3", sr_interval: "6" })),
    ).toBe("due 2026-07-20");
  });

  // Every other row in the listing states a standing, so a row left with none
  // reads as a fact the surface knows and is withholding.
  it("says a graded term carries no due date rather than saying nothing", () => {
    expect(dueStanding(term({ sr_reps: "3" }))).toBe("no due date recorded");
  });
});
