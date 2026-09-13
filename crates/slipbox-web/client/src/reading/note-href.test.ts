import { describe, expect, it } from "vitest";

import { referenceOf, type LinkTarget } from "../org/navigation.jsx";
import { ADDRESSES_ONLY, noteHref } from "./note-href.js";
import { decodeStack } from "./stack.js";

describe("noteHref", () => {
  it("emits the router's own URL for an id target, decodable back to it", () => {
    const target: LinkTarget = { id: "abc-123", target: "id:abc-123" };
    const href = noteHref(target);

    expect(href).toBe("?note=id%3Aabc-123");
    expect(decodeStack(href)).toEqual([referenceOf(target)]);
  });

  it("emits the router's own URL for a raw-key target, decodable back to it", () => {
    const target: LinkTarget = { id: null, target: "notes/a.org::0" };
    const href = noteHref(target);

    expect(decodeStack(href)).toEqual(["notes/a.org::0"]);
  });
});

describe("ADDRESSES_ONLY", () => {
  it("addresses a note while honoring no verb", () => {
    const target: LinkTarget = { id: "abc-123", target: "id:abc-123" };

    expect(ADDRESSES_ONLY.href(target)).toBe(noteHref(target));
    expect(() => {
      ADDRESSES_ONLY.glance(null);
      ADDRESSES_ONLY.pin(target);
      ADDRESSES_ONLY.go(target);
    }).not.toThrow();
  });
});
