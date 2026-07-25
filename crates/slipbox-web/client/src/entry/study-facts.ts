/*
 * A term's SM-2 review drawer, carried as text on the node record (`sr_due`,
 * `sr_interval`, `sr_reps`, `sr_ease`, `sr_last`), formatted as an ordered
 * read-only list of facts.
 */

import type { NodeRecord } from "../api/types.js";

/** One labelled review fact, ready to render. */
export interface StudyFact {
  readonly label: string;
  readonly value: string;
}

/** A day count as a natural phrase: "1 day" / "21 days". */
function days(value: string): string {
  return value === "1" ? "1 day" : `${value} days`;
}

/**
 * The review facts of a term, in display order, omitting any field the term does
 * not carry. Values are surfaced as stored: the server owns the canonical
 * spelling of dates and numbers.
 */
export function studyFacts(term: NodeRecord): StudyFact[] {
  const facts: StudyFact[] = [];
  if (term.sr_due) {
    facts.push({ label: "Due", value: term.sr_due });
  }
  if (term.sr_interval) {
    facts.push({ label: "Interval", value: days(term.sr_interval) });
  }
  if (term.sr_reps) {
    facts.push({ label: "Reviews", value: term.sr_reps });
  }
  if (term.sr_ease) {
    facts.push({ label: "Ease", value: term.sr_ease });
  }
  if (term.sr_last) {
    facts.push({ label: "Last reviewed", value: term.sr_last });
  }
  return facts;
}
