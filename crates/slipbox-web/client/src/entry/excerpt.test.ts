import { describe, expect, it } from "vitest";

import type { ContentSegment } from "../api/types.js";
import type { Inline } from "../org/types.js";
import { excerptRuns } from "./excerpt.js";

/* The characters the module splices around a matched run. */
const OPEN = "\u0002";
const CLOSE = "\u0003";

const plain = (text: string): ContentSegment => ({ text, matched: false });
const match = (text: string): ContentSegment => ({ text, matched: true });

function text(nodes: readonly Inline[]): string {
  return nodes
    .map((node) => {
      switch (node.type) {
        case "text":
          return node.value;
        case "bold":
        case "italic":
          return text(node.children);
        case "verbatim":
          return node.value;
        case "math":
          return node.tex;
        case "link":
          return text(node.label);
      }
    })
    .join("");
}

const flat = (segments: ContentSegment[]): string =>
  excerptRuns(segments)
    .map((run) => text(run.prose))
    .join("");

const highlighted = (segments: ContentSegment[]): string[] =>
  excerptRuns(segments)
    .filter((run) => run.matched)
    .map((run) => text(run.prose));

describe("excerptRuns", () => {
  it("reduces a link to its label, dropping the id target", () => {
    const segments = [
      plain("a failure mode of [[id:17209c11-7e35][variational autoencoders]] in "),
      match("training"),
    ];
    expect(flat(segments)).toBe(
      "a failure mode of variational autoencoders in training",
    );
    expect(highlighted(segments)).toEqual(["training"]);
  });

  it("renders emphasis as emphasis rather than its markers", () => {
    const runs = excerptRuns([plain("/Posterior collapse/ is common")]);
    expect(flat([plain("/Posterior collapse/ is common")])).toBe(
      "Posterior collapse is common",
    );
    expect(runs[0]?.prose[0]).toEqual({
      type: "italic",
      children: [{ type: "text", value: "Posterior collapse" }],
    });
  });

  it("keeps a formula as math rather than its TeX source", () => {
    const runs = excerptRuns([plain("bounded by \\(x^2\\) above")]);
    const math = runs
      .flatMap((run) => run.prose)
      .find((node) => node.type === "math");
    expect(math).toEqual({ type: "math", tex: "x^2" });
  });

  it("highlights a run that the reduction moves, inside a link label", () => {
    const segments = [
      plain("see [[id:abc][the "),
      match("ELBO"),
      plain(" derivation]] there"),
    ];
    expect(flat(segments)).toBe("see the ELBO derivation there");
    expect(highlighted(segments)).toEqual(["ELBO"]);
  });

  it("keeps words emphasized when a highlight cuts an emphasized phrase", () => {
    const segments = [plain("/exactly "), match("half"), plain(" italic/")];
    const runs = excerptRuns(segments);
    expect(flat(segments)).toBe("exactly half italic");
    expect(highlighted(segments)).toEqual(["half"]);
    for (const run of runs) {
      expect(run.prose.every((node) => node.type === "italic")).toBe(true);
    }
  });

  it("reads adjacent matched segments as one highlight", () => {
    expect(highlighted([match("global "), match("maximization")])).toEqual([
      "global maximization",
    ]);
  });

  it("leaves prose that looks like markup as the characters it spells", () => {
    const segments = [plain("<mark>not a highlight</mark> and "), match("markup")];
    expect(flat(segments)).toBe("<mark>not a highlight</mark> and markup");
    expect(highlighted(segments)).toEqual(["markup"]);
  });

  it("cannot be made to forge a highlight by a note that spells the markers", () => {
    const segments = [plain(`a ${OPEN}forged${CLOSE} highlight and `), match("real")];
    expect(flat(segments)).toBe("a forged highlight and real");
    expect(highlighted(segments)).toEqual(["real"]);
  });

  it("keeps the excerpt whole when a marker lands where the reduction cuts", () => {
    const segments = [
      plain("see [[id:"),
      match("abc"),
      plain("][the label]] after"),
    ];
    expect(flat(segments)).toBe("see the label after");
  });

  it("highlights a whole formula rather than slicing its TeX", () => {
    const segments = [plain("bounded by \\("), match("x^2"), plain("\\) above")];
    const math = excerptRuns(segments)
      .flatMap((run) => run.prose)
      .find((node) => node.type === "math");
    expect(math).toEqual({ type: "math", tex: "x^2" });
  });

  it("ends the excerpt where the server's elision cut a link in half", () => {
    expect(flat([plain("addresses the likelihood of [[id:b263a471…")])).toBe(
      "addresses the likelihood of…",
    );
  });

  it("ends the excerpt where the elision cut a formula in half", () => {
    expect(flat([plain("maximise the posterior \\(p(\\mathbf{X} \\mid…")])).toBe(
      "maximise the posterior…",
    );
  });

  it("closes a highlight left open by a cut tail", () => {
    const segments = [
      plain("the "),
      match("intractable marginal likelihood of [[id:abc…"),
    ];
    expect(flat(segments)).toBe("the intractable marginal likelihood of…");
    expect(highlighted(segments)).toEqual(["intractable marginal likelihood of"]);
  });

  it("leaves a whole construct alone when a later one is cut", () => {
    const segments = [plain("both \\(p(x)\\) and the posterior \\(p(z \\mid…")];
    expect(flat(segments)).toBe("both p(x) and the posterior…");
  });

  it("starts the excerpt where the elision cut a formula's opener away", () => {
    expect(
      flat([plain("…\\mathbf{w},t)\\) and the forward process mean")]),
    ).toBe("…and the forward process mean");
  });

  it("keeps a cut link's description, which is the words the reader would see", () => {
    // The cut fell inside the target, and the `][` survived it.
    expect(
      flat([plain("…0ada3238-a90d][expectation–maximisation]] is a technique")]),
    ).toBe("…expectation–maximisation is a technique");
  });

  it("keeps what a cut through a link's description left of it", () => {
    // No `][` survived: the cut fell past it, inside the description.
    expect(flat([plain("…variational autoencoder]] is trained by maximising")])).toBe(
      "…variational autoencoder is trained by maximising",
    );
  });

  it("keeps a description the cut left flush against its own bracket", () => {
    // The cut fell between the target's `]` and the description's `[`.
    expect(flat([plain("…[expectation–maximisation]] is a technique")])).toBe(
      "…expectation–maximisation is a technique",
    );
  });

  it("keeps a highlight the head cut left half-open", () => {
    const segments = [
      plain("…\\mathbf{x})\\) and the "),
      match("posterior"),
      plain(" mean"),
    ];
    expect(flat(segments)).toBe("…and the posterior mean");
    expect(highlighted(segments)).toEqual(["posterior"]);
  });

  it("reopens a highlight the head cut swallowed the start of", () => {
    const segments = [match("…\\mathbf{x})\\) and the posterior"), plain(" mean")];
    expect(flat(segments)).toBe("…and the posterior mean");
    expect(highlighted(segments)).toEqual(["and the posterior"]);
  });

  it("starts after a display formula's closer, keeping the prose it precedes", () => {
    expect(
      flat([plain("…C_k).\n\\]\nGiven \\(p(C_k)\\), Bayes' rule turns this")]),
    ).toBe("…Given p(C_k), Bayes' rule turns this");
  });

  it("takes the outer cut when the elision left two stray closers", () => {
    expect(flat([plain("…x)\\) then \\mathbf{z}\\) and the prior")])).toBe(
      "…and the prior",
    );
  });

  it("cuts both ends of an excerpt sliced through a construct twice", () => {
    expect(
      flat([plain("…z}_t)\\) matches the forward posterior \\(q(\\mathbf{z}…")]),
    ).toBe("…matches the forward posterior…");
  });

  it("leaves a closer alone when its own opener is in the excerpt", () => {
    expect(flat([plain("bounded by \\(x^2\\) above")])).toBe("bounded by x^2 above");
  });

  it("has no runs for an excerpt the server left empty", () => {
    expect(excerptRuns([])).toEqual([]);
  });

  it("collapses the newlines a body excerpt spans into single spaces", () => {
    expect(flat([plain("one line\nand   the  next")])).toBe(
      "one line and the next",
    );
  });
});
