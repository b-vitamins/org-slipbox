import { render } from "@solidjs/testing-library";
import { describe, expect, it } from "vitest";

import { DisplayMath, InlineMath } from "./Math.jsx";

/** TeX nested deeply enough to exhaust KaTeX's recursive descent parser. */
const RUNAWAY = "\\sqrt{".repeat(2000);

describe("math rendering", () => {
  it("renders a formula KaTeX understands", () => {
    const { container } = render(() => <InlineMath tex="E = mc^2" />);

    expect(container.querySelector(".katex")).not.toBeNull();
    expect(container.querySelector("annotation")?.textContent).toBe("E = mc^2");
  });

  // `throwOnError: false` covers this one: KaTeX rejects the TeX and renders it
  // as its own source in the error color.
  it("degrades TeX it cannot parse to the raw source", () => {
    const { container } = render(() => <InlineMath tex="\frac{1" />);

    const error = container.querySelector(".katex-error");
    expect(error?.textContent).toBe("\\frac{1");
  });

  // This one it does not: the parser runs out of stack, which is not KaTeX's
  // error to suppress. A formula must cost the reader the formula, never the
  // column it sits in, so the outcome is the same as any other bad TeX.
  it("degrades TeX that defeats the parser the same way", () => {
    const { container } = render(() => <InlineMath tex={RUNAWAY} />);

    const error = container.querySelector(".katex-error");
    expect(error?.textContent).toBe(RUNAWAY);
    expect(container.textContent).toContain("\\sqrt{");
  });

  it("degrades a display formula that defeats the parser too", () => {
    const { container } = render(() => <DisplayMath tex={RUNAWAY} />);

    expect(container.querySelector(".katex-error")?.textContent).toBe(RUNAWAY);
  });
});
