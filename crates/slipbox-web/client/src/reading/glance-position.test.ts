import { describe, expect, it } from "vitest";

import { placeGlance, type Rect } from "./glance-position.js";

/** Build a rectangle from a top-left corner and a size. */
function rect(left: number, top: number, width: number, height: number): Rect {
  return { left, top, width, height, right: left + width, bottom: top + height };
}

const CARD = { width: 340, height: 120 };
const VIEWPORT = { width: 1280, height: 800 };

describe("placeGlance", () => {
  it("floats just below and left-aligned to the link when it fits", () => {
    const placement = placeGlance(rect(200, 100, 80, 20), CARD, VIEWPORT);
    expect(placement.above).toBe(false);
    expect(placement.top).toBe(128); // bottom (120) + gap (8)
    expect(placement.left).toBe(200);
  });

  it("flips above when the card would overflow the bottom edge", () => {
    // A link near the bottom leaves no room below for a 120px card.
    const placement = placeGlance(rect(200, 740, 80, 20), CARD, VIEWPORT);
    expect(placement.above).toBe(true);
    expect(placement.top).toBe(612); // top (740) - gap (8) - height (120)
  });

  it("shifts left so the card stays within the right edge", () => {
    // A link hard against the right edge: left is clamped to keep the card in.
    const placement = placeGlance(rect(1260, 100, 20, 20), CARD, VIEWPORT);
    expect(placement.left).toBe(VIEWPORT.width - CARD.width - 12); // 928
  });

  it("never places the card left of the viewport margin", () => {
    const placement = placeGlance(rect(-40, 100, 20, 20), CARD, VIEWPORT);
    expect(placement.left).toBe(12);
  });

  it("keeps a card too tall for either side within the viewport", () => {
    // A card taller than the room above and below its link fits on neither side.
    // Clamping keeps its top edge on screen, so the reader sees the opening of
    // the excerpt rather than a card hanging off the bottom.
    const tall = { width: 340, height: 700 };
    const placement = placeGlance(rect(200, 300, 80, 20), tall, VIEWPORT);
    expect(placement.above).toBe(false);
    expect(placement.top).toBe(VIEWPORT.height - tall.height - 12); // 88
  });

  it("never places the card above the viewport margin", () => {
    // Taller than the whole viewport: the clamp's own range is inverted, and the
    // top margin wins so the card starts where it can be read.
    const huge = { width: 340, height: 900 };
    const placement = placeGlance(rect(200, 400, 80, 20), huge, VIEWPORT);
    expect(placement.top).toBe(12);
  });
});
