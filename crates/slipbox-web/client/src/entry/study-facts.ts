import type { NodeRecord } from "../api/types.js";

export interface StudyFact {
  readonly label: string;
  readonly value: string;
}

const SCHEDULE_FIELDS = [
  "sr_due",
  "sr_interval",
  "sr_reps",
  "sr_ease",
  "sr_last",
] as const;

function days(value: string): string {
  return value === "1" ? "1 day" : `${value} days`;
}

export function neverReviewed(term: NodeRecord): boolean {
  return SCHEDULE_FIELDS.every((field) => !term[field]);
}

export function dueStanding(term: NodeRecord): string {
  if (neverReviewed(term)) {
    return "never reviewed";
  }
  return term.sr_due ? `due ${term.sr_due}` : "no due date recorded";
}

/** Return the stored SM-2 facts in display order. */
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
