import { describe, expect, it } from "vitest";

import { followableHref } from "./link-target.js";

describe("followableHref", () => {
  it("follows the schemes that navigate", () => {
    expect(followableHref("https://example.org/a")).toBe(
      "https://example.org/a",
    );
    expect(followableHref("http://example.org/a")).toBe("http://example.org/a");
    expect(followableHref("mailto:reader@example.org")).toBe(
      "mailto:reader@example.org",
    );
  });

  it("reads a scheme case-insensitively, as a browser does", () => {
    expect(followableHref("HTTPS://example.org")).toBe("HTTPS://example.org");
  });

  it("refuses a scheme that executes in the reading origin", () => {
    expect(followableHref("javascript:alert(1)")).toBeNull();
    expect(followableHref("JaVaScRiPt:alert(1)")).toBeNull();
    expect(followableHref("vbscript:msgbox(1)")).toBeNull();
  });

  it("refuses a scheme that carries its own document", () => {
    expect(followableHref("data:text/html,<script>alert(1)</script>")).toBeNull();
    expect(followableHref("blob:http://localhost:8080/abc")).toBeNull();
  });

  it("refuses a scheme split by the characters a browser strips", () => {
    expect(followableHref("java\tscript:alert(1)")).toBeNull();
    expect(followableHref("java\nscript:alert(1)")).toBeNull();
    expect(followableHref("java\rscript:alert(1)")).toBeNull();
    expect(followableHref("j\ta\nv\ra\tscript:alert(1)")).toBeNull();
  });

  it("refuses a scheme hidden behind leading whitespace or controls", () => {
    expect(followableHref("   javascript:alert(1)")).toBeNull();
    expect(followableHref("\u0000javascript:alert(1)")).toBeNull();
    expect(followableHref("\n \tjavascript:alert(1)")).toBeNull();
  });

  it("refuses a target this surface cannot reach over HTTP", () => {
    expect(followableHref("file:/home/reader/notes.org")).toBeNull();
    expect(followableHref("./sibling.org")).toBeNull();
    expect(followableHref("/absolute/path")).toBeNull();
    expect(followableHref("notes.org")).toBeNull();
    expect(followableHref("")).toBeNull();
  });

  it("returns the normalized target rather than the raw one", () => {
    expect(followableHref("  https://example.org/a  ")).toBe(
      "https://example.org/a",
    );
    expect(followableHref("https://example.org/\ta")).toBe(
      "https://example.org/a",
    );
  });
});
