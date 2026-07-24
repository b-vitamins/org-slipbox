import { describe, expect, it } from "vitest";

import { slipboxName } from "./identity.js";

describe("slipboxName", () => {
  it("names the slipbox by the last segment of an absolute root", () => {
    expect(slipboxName("/home/reader/notes")).toBe("notes");
  });

  it("ignores a trailing slash", () => {
    expect(slipboxName("/home/reader/notes/")).toBe("notes");
  });

  it("passes through a bare name with no path", () => {
    expect(slipboxName("notes")).toBe("notes");
  });

  it("does not leak the parent directories of the root", () => {
    expect(slipboxName("/Users/ayan/org/myslipbox")).toBe("myslipbox");
  });

  it("falls back to the raw root when there is no segment to show", () => {
    expect(slipboxName("/")).toBe("/");
  });
});
