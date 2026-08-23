/*
 * A term's SM-2 review drawer, carried as text on the node record (`sr_due`,
 * `sr_interval`, `sr_reps`, `sr_ease`, `sr_last`), formatted as an ordered
 * read-only list of facts, plus the one-line standing a listing row shows.
 */

import type { NodeRecord } from "../api/types.js";

/** One labelled review fact, ready to render. */
export interface StudyFact {
  readonly label: string;
  readonly value: string;
}

/** The drawer's fields, which grading writes and nothing else does. */
const SCHEDULE_FIELDS = [
  "sr_due",
  "sr_interval",
  "sr_reps",
  "sr_ease",
  "sr_last",
] as const;

/** A day count as a natural phrase: "1 day" / "21 days". */
function days(value: string): string {
  return value === "1" ? "1 day" : `${value} days`;
}

/**
 * Whether a term's review drawer holds nothing to report. Every field arrives with
 * a grading, so an empty drawer is a term no review has landed on. Emptiness is
 * read the way each fact is read, by truthiness, so a `0` counts as the absence it
 * stands for.
 */
export function neverReviewed(term: NodeRecord): boolean {
  return SCHEDULE_FIELDS.every((field) => !term[field]);
}

/**
 * Why a term is standing in the due listing, in the words its drawer supports. A
 * term due for want of a first review is named by that rather than by a date
 * grading has yet to write, and a drawer holding a review but no date names the
 * fact it is missing: every row states a standing, so a row stating none reads as
 * a fact the surface has and is keeping back.
 */
export function dueStanding(term: NodeRecord): string {
  if (neverReviewed(term)) {
    return "never reviewed";
  }
  return term.sr_due ? `due ${term.sr_due}` : "no due date recorded";
}

/**
 * The review facts of a term, in display order, omitting any field the term does
 * not carry. Values are surfaced as stored: the server owns the canonical
 * spelling of dates and numbers. An empty drawer is a fact of its own, since a
 * pane rendering nothing left the reader to guess at the silence.
 */
export function studyFacts(term: NodeRecord): StudyFact[] {
  if (neverReviewed(term)) {
    return [{ label: "Schedule", value: "Never reviewed" }];
  }
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
