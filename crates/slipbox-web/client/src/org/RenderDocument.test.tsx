import { render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";

import { NavigationProvider, type Navigation } from "./navigation.jsx";
import { parseOrg } from "./parse.js";
import { RenderDocument } from "./RenderDocument.jsx";

describe("RenderDocument", () => {
  it("renders paragraphs, emphasis, and headings", () => {
    const doc = parseOrg("* Section\n\nA /clear/ and *bold* line.");
    render(() => <RenderDocument document={doc} />);

    expect(
      screen.getByRole("heading", { name: "Section" }),
    ).toBeInTheDocument();
    expect(screen.getByText("clear").tagName).toBe("EM");
    expect(screen.getByText("bold").tagName).toBe("STRONG");
  });

  it("starts body headings at the level below a column's own title", () => {
    const doc = parseOrg("* Section\n\n** Nested\n");
    render(() => <RenderDocument document={doc} />);

    expect(
      screen.getByRole("heading", { level: 2, name: "Section" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 3, name: "Nested" }),
    ).toBeInTheDocument();
  });

  it("nests body headings under a surface that heads the document deeper", () => {
    const doc = parseOrg("* Section\n\n** Nested\n\n***** Deepest\n");
    render(() => <RenderDocument document={doc} baseLevel={3} />);

    expect(
      screen.getByRole("heading", { level: 3, name: "Section" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 4, name: "Nested" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 6, name: "Deepest" }),
    ).toBeInTheDocument();
  });

  it("renders inline math through KaTeX", () => {
    const doc = parseOrg("mass \\\\(E = mc^2\\\\) done");
    const { container } = render(() => <RenderDocument document={doc} />);

    // KaTeX emits a `.katex` root and an accessible MathML annotation.
    const katex = container.querySelector(".katex");
    expect(katex).not.toBeNull();
    expect(container.querySelector("annotation")?.textContent).toBe("E = mc^2");
  });

  it("renders a quoted passage as a blockquote around its prose", () => {
    const doc = parseOrg("#+begin_quote\nThe map is not the territory.\n#+end_quote");
    const { container } = render(() => <RenderDocument document={doc} />);

    const quote = container.querySelector("blockquote.org-quote");
    expect(quote).not.toBeNull();
    expect(quote?.querySelector("p")?.textContent).toBe(
      "The map is not the territory.",
    );
  });

  it("pins an id link on a plain click instead of following the browser", () => {
    const navigation: Navigation = {
      glance: vi.fn(),
      pin: vi.fn(),
      go: vi.fn(),
    };
    const doc = parseOrg("see [[id:abc-123][the algorithm]] now");

    render(() => (
      <NavigationProvider navigation={navigation}>
        <RenderDocument document={doc} />
      </NavigationProvider>
    ));

    const link = screen.getByText("the algorithm").closest("a")!;
    // The href is the real URL the router reads, not a decorative hash.
    expect(link.getAttribute("href")).toBe("?note=id%3Aabc-123");
    link.dispatchEvent(
      new MouseEvent("click", { bubbles: true, cancelable: true }),
    );

    expect(navigation.pin).toHaveBeenCalledTimes(1);
    expect(
      (navigation.pin as ReturnType<typeof vi.fn>).mock.calls[0]![0],
    ).toEqual({
      id: "abc-123",
      target: "id:abc-123",
    });
    expect(navigation.go).not.toHaveBeenCalled();
  });

  it("escalates to go on an alt-click", () => {
    const navigation: Navigation = {
      glance: vi.fn(),
      pin: vi.fn(),
      go: vi.fn(),
    };
    const doc = parseOrg("see [[id:abc-123][the algorithm]] now");

    render(() => (
      <NavigationProvider navigation={navigation}>
        <RenderDocument document={doc} />
      </NavigationProvider>
    ));

    const link = screen.getByText("the algorithm").closest("a")!;
    link.dispatchEvent(
      new MouseEvent("click", {
        bubbles: true,
        cancelable: true,
        altKey: true,
      }),
    );

    expect(navigation.go).toHaveBeenCalledTimes(1);
    expect(navigation.pin).not.toHaveBeenCalled();
  });

  it("renders an external link as an anchor the browser follows", () => {
    const doc = parseOrg("see [[https://example.org/a][the paper]] now");
    render(() => <RenderDocument document={doc} />);

    const link = screen.getByText("the paper").closest("a");
    expect(link?.getAttribute("href")).toBe("https://example.org/a");
  });

  it("renders a script-bearing link target as inert text, not an anchor", () => {
    const doc = parseOrg("see [[javascript:alert(1)][the paper]] now");
    const { container } = render(() => <RenderDocument document={doc} />);

    expect(container.querySelector("a")).toBeNull();
    const inert = screen.getByText("the paper");
    expect(inert.tagName).toBe("SPAN");
    expect(inert).toHaveClass("org-link--inert");
  });

  it("renders a source block verbatim with its language and a copy control", () => {
    const doc = parseOrg("#+begin_src python\nprint(1)\n#+end_src");
    const { container } = render(() => <RenderDocument document={doc} />);

    const figure = container.querySelector("figure.org-src");
    expect(figure?.getAttribute("data-lang")).toBe("python");
    expect(container.querySelector(".org-src__code")?.textContent).toBe(
      "print(1)",
    );

    // The language is surfaced and a copy control is present and labelled.
    expect(container.querySelector(".org-src__lang")?.textContent).toBe(
      "python",
    );
    expect(
      screen.getByRole("button", { name: "Copy code to clipboard" }),
    ).toBeInTheDocument();
  });

  it("renders a table header as th and typesets math cells through KaTeX", () => {
    const doc = parseOrg(
      "| quantity | value |\n|---+---|\n| \\\\(d\\\\) | 0 |",
    );
    const { container } = render(() => <RenderDocument document={doc} />);

    // The pre-rule row is a real header, not another body row.
    const headers = container.querySelectorAll("thead th");
    expect(headers).toHaveLength(2);
    expect(headers[0]?.textContent).toBe("quantity");
    expect(container.querySelectorAll("tbody td")).toHaveLength(2);

    // The math cell renders through KaTeX rather than printing raw TeX.
    const bodyCell = container.querySelector("tbody td");
    expect(bodyCell?.querySelector(".katex")).not.toBeNull();
    expect(bodyCell?.textContent).not.toContain("\\(");
  });
});
