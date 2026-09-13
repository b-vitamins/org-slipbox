/*
 * Given the same source and the same href grammar, the web reading column and the
 * standalone bundle must produce the same subtree.
 */

import { render } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";

import { NavigationProvider } from "../org/navigation.jsx";
import { parseOrg } from "../org/parse.js";
import { RenderDocument } from "../org/RenderDocument.jsx";
import { ADDRESSES_ONLY, noteHref } from "../reading/note-href.js";
import { mountOrgDocument } from "./mount.jsx";

const SOURCE = [
  "A paragraph with *bold*, /italic/, =verbatim= and inline math \\(e^{i\\pi} = -1\\).",
  "",
  "It links to [[id:abc-123][a note]], to [[https://example.org/a][a paper]],",
  "and to [[file:notes/other.org][a file it cannot follow]].",
  "",
  "* A heading",
  "",
  "- a plain item",
  "- another, with \\(x^2\\) in it",
  "",
  "1. an ordered item",
  "2. and the next",
  "",
  "#+begin_src rust",
  "fn main() {",
  '    println!("hello");',
  "}",
  "#+end_src",
  "",
  "#+begin_example",
  "literal <b>text</b>",
  "#+end_example",
  "",
  "#+begin_quote",
  "A quoted line with =code=.",
  "#+end_quote",
  "",
  "\\[ \\int_0^1 x^2 \\, dx = \\frac{1}{3} \\]",
  "",
  "| quantity | value |",
  "|----------+-------|",
  "| \\(d\\)    | 0     |",
  "",
  "** A nested heading",
  "",
  "Broken math \\(\\frac{1\\) stays raw.",
  "",
].join("\n");

function webDocument(): string {
  const { container } = render(() => (
    <NavigationProvider navigation={ADDRESSES_ONLY}>
      <RenderDocument document={parseOrg(SOURCE)} />
    </NavigationProvider>
  ));
  return (container.querySelector(".org-document") as HTMLElement).outerHTML;
}

function standaloneDocument(): string {
  const host = document.createElement("div");
  document.body.append(host);
  mountOrgDocument(host, {
    content: { source: SOURCE },
    href: (link) => noteHref({ id: link.id, target: link.target }),
  });
  return (host.querySelector(".org-document") as HTMLElement).outerHTML;
}

describe("the shared Org implementation", () => {
  afterEach(() => {
    document.body.replaceChildren();
  });

  it("renders one source identically through both entry points", () => {
    expect(standaloneDocument()).toBe(webDocument());
  });

  it("renders every construct the source exercises", () => {
    const web = webDocument();

    expect(web).toContain("org-heading");
    expect(web).toContain("org-src__code");
    expect(web).toContain("org-table");
    expect(web).toContain("org-quote");
    expect(web).toContain("katex");
    expect(web).toContain('href="?note=id%3Aabc-123"');
    expect(web).toContain("org-link--inert");
  });
});
