import { afterEach, describe, expect, it } from "vitest";

import { mountOrgSearch } from "./search.jsx";

describe("mountOrgSearch", () => {
  afterEach(() => {
    document.body.replaceChildren();
  });

  it("renders result prose instead of exposing Org, identifiers, or TeX source", () => {
    const host = document.createElement("div");
    document.body.append(host);
    const opened: string[] = [];
    const handle = mountOrgSearch(host, {
      results: [
        {
          key: "id:private-identity",
          title: "Control systems",
          tags: ["control"],
          term: false,
          excerpt: [
            {
              text: "A [[id:another-private-identity][control input]] \\(u(t)\\) drives the ",
              matched: false,
            },
            { text: "system", matched: true },
            { text: ".", matched: false },
          ],
        },
      ],
      theme: "light",
      onOpen: (key) => opened.push(key),
    });

    const result = host.querySelector(".org-search-result") as HTMLElement;
    expect(result.textContent).toContain("Control systems");
    expect(result.textContent).toContain("control input");
    expect(result.textContent).not.toContain("private-identity");
    expect(result.innerHTML).not.toContain("[[id:");
    expect(result.innerHTML).not.toContain("\\(");
    expect(result.querySelector(".katex")).not.toBeNull();
    expect(result.querySelector("mark")?.textContent).toBe("system");

    (result.querySelector("button") as HTMLButtonElement).click();
    expect(opened).toEqual(["id:private-identity"]);

    handle.dispose();
    expect(host.children).toHaveLength(0);
  });
});
