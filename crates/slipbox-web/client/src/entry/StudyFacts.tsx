/*
 * A glossary term's SM-2 review schedule as a read-only definition list, built
 * by the pure `studyFacts` formatter. Every term has facts: one whose drawer is
 * empty has the fact that it has never been reviewed.
 */

import { For, type Component } from "solid-js";

import type { NodeRecord } from "../api/types.js";
import { studyFacts } from "./study-facts.js";

export const StudyFacts: Component<{ term: NodeRecord }> = (props) => {
  const facts = (): ReturnType<typeof studyFacts> => studyFacts(props.term);
  return (
    <dl class="study-facts" aria-label="Review schedule">
      <For each={facts()}>
        {(fact) => (
          <div class="study-facts__fact">
            <dt class="study-facts__label">{fact.label}</dt>
            <dd class="study-facts__value">{fact.value}</dd>
          </div>
        )}
      </For>
    </dl>
  );
};
