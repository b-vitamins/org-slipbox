import { describe, expect, it } from "vitest";

import { followableHref, resolvedAssetHref } from "./link-target.js";

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

describe("resolvedAssetHref", () => {
  it("serves an asset the host placed beside the document", () => {
    expect(resolvedAssetHref("assets/diagram.png")).toBe("assets/diagram.png");
    expect(resolvedAssetHref("./assets/diagram.png")).toBe("./assets/diagram.png");
    expect(resolvedAssetHref("../shared/diagram.png")).toBe("../shared/diagram.png");
    expect(resolvedAssetHref("/media/diagram.png")).toBe("/media/diagram.png");
  });

  it("serves an asset the host addressed by scheme", () => {
    expect(resolvedAssetHref("https://example.org/a.png")).toBe(
      "https://example.org/a.png",
    );
    expect(resolvedAssetHref("blob:http://localhost:4174/abc")).toBe(
      "blob:http://localhost:4174/abc",
    );
  });

  it("refuses a resolution that would execute or carry its own document", () => {
    expect(resolvedAssetHref("javascript:alert(1)")).toBeNull();
    expect(resolvedAssetHref("java\tscript:alert(1)")).toBeNull();
    expect(resolvedAssetHref("  JaVaScRiPt:alert(1)")).toBeNull();
    expect(resolvedAssetHref("data:text/html,<script>alert(1)</script>")).toBeNull();
    expect(resolvedAssetHref("vbscript:msgbox(1)")).toBeNull();
  });

  it("refuses a resolution that leaves the document's own scheme behind", () => {
    expect(resolvedAssetHref("//example.org/a.png")).toBeNull();
    expect(resolvedAssetHref("  //example.org/a.png")).toBeNull();
  });

  it("refuses an authority a browser would read through backslashes", () => {
    expect(resolvedAssetHref("\\\\example.org/plot.png")).toBeNull();
    expect(resolvedAssetHref("/\\example.org/plot.png")).toBeNull();
    expect(resolvedAssetHref("\\/example.org/plot.png")).toBeNull();
    expect(resolvedAssetHref("\t\\\\example.org/plot.png")).toBeNull();
    expect(resolvedAssetHref("/\t/example.org/plot.png")).toBeNull();
    // Ambiguous even where it names no authority, so it is refused outright.
    expect(resolvedAssetHref("media\\plot.png")).toBeNull();
    expect(resolvedAssetHref("https://example.org\\@evil.example/a.png")).toBeNull();
  });

  it("refuses an empty resolution rather than emitting a self-link", () => {
    expect(resolvedAssetHref("")).toBeNull();
    expect(resolvedAssetHref("   ")).toBeNull();
  });
});
