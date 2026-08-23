import { describe, expect, it } from "vitest";

import { placeGlance, type Rect, type Span } from "./glance-position.js";

/** Build a rectangle from a top-left corner and a size. */
function rect(left: number, top: number, width: number, height: number): Rect {
  return { left, top, width, height, right: left + width, bottom: top + height };
}

const CARD = { width: 340, height: 120 };
const VIEWPORT = { width: 1280, height: 800 };
const FILLED: Span = { left: 0, right: VIEWPORT.width };

const WIDE = { width: 1440, height: 800 };
const ONE_COLUMN: Span = { left: 407.5, right: 1032.5 };

describe("placeGlance beside the reading area", () => {
  it("stands clear of the columns when the space beside them holds the card", () => {
    const placement = placeGlance(rect(500, 300, 80, 20), CARD, WIDE, ONE_COLUMN);

    expect(placement.left).toBeGreaterThanOrEqual(ONE_COLUMN.right);
    expect(placement.left + CARD.width).toBeLessThanOrEqual(WIDE.width - 12);
    expect(placement.top).toBe(300);
    expect(placement.above).toBe(false);
  });

  it("takes the leading side when the trailing one is too narrow", () => {
    const offset: Span = { left: 360, right: 1200 };

    const placement = placeGlance(rect(500, 300, 80, 20), CARD, WIDE, offset);

    expect(placement.left + CARD.width).toBeLessThanOrEqual(offset.left);
    expect(placement.left).toBe(12);
  });

  it("shrinks into a usable 1280px gutter instead of covering prose", () => {
    const oneCentered: Span = { left: 327.5, right: 952.5 };
    const placement = placeGlance(
      rect(500, 300, 80, 20),
      CARD,
      VIEWPORT,
      oneCentered,
    );

    expect(placement.left).toBe(960.5);
    expect(placement.width).toBe(307.5);
    expect(placement.left).toBeGreaterThan(oneCentered.right);
    expect(placement.left + placement.width).toBe(VIEWPORT.width - 12);
    expect(placement.top).toBe(300);
  });

  it("keeps the placement below the link where a second column holds the space", () => {
    const twoColumns: Span = { left: 95, right: 1345 };

    const placement = placeGlance(rect(200, 300, 80, 20), CARD, WIDE, twoColumns);

    expect(placement.top).toBe(328); // bottom (320) + gap (8)
    expect(placement.left).toBe(200);
  });

  it("keeps a card beside the first and the last line inside the viewport", () => {
    const first = placeGlance(rect(500, 4, 80, 20), CARD, WIDE, ONE_COLUMN);
    expect(first.top).toBe(12);

    const last = placeGlance(rect(500, 780, 80, 20), CARD, WIDE, ONE_COLUMN);
    expect(last.top).toBe(WIDE.height - CARD.height - 12); // 668
  });

  it("names no space beside a reading area of no width", () => {
    const unmeasured: Span = { left: 0, right: 0 };

    const placement = placeGlance(rect(200, 300, 80, 20), CARD, WIDE, unmeasured);

    expect(placement.left).toBe(200);
    expect(placement.top).toBe(328); // bottom (320) + gap (8)
  });

  it("re-places a loaded card in the region the empty one took", () => {
    const anchor = rect(500, 300, 80, 20);
    const loading = placeGlance(anchor, { width: 340, height: 40 }, WIDE, ONE_COLUMN);
    const loaded = placeGlance(anchor, { width: 340, height: 167 }, WIDE, ONE_COLUMN);

    expect(loaded.left).toBe(loading.left);
    expect(loaded.top).toBe(loading.top);
  });
});

describe("placeGlance against the link", () => {
  it("floats just below and left-aligned to the link when it fits", () => {
    const placement = placeGlance(rect(200, 100, 80, 20), CARD, VIEWPORT, FILLED);
    expect(placement.above).toBe(false);
    expect(placement.top).toBe(128); // bottom (120) + gap (8)
    expect(placement.left).toBe(200);
    expect(placement.width).toBe(CARD.width);
  });

  it("flips above when the card would overflow the bottom edge", () => {
    // A link near the bottom leaves no room below for a 120px card.
    const placement = placeGlance(rect(200, 740, 80, 20), CARD, VIEWPORT, FILLED);
    expect(placement.above).toBe(true);
    expect(placement.top).toBe(612); // top (740) - gap (8) - height (120)
  });

  it("shifts left so the card stays within the right edge", () => {
    // A link hard against the right edge: left is clamped to keep the card in.
    const placement = placeGlance(rect(1260, 100, 20, 20), CARD, VIEWPORT, FILLED);
    expect(placement.left).toBe(VIEWPORT.width - CARD.width - 12); // 928
  });

  it("never places the card left of the viewport margin", () => {
    const placement = placeGlance(rect(-40, 100, 20, 20), CARD, VIEWPORT, FILLED);
    expect(placement.left).toBe(12);
  });

  it("shrinks between the margins on a viewport narrower than the card", () => {
    const viewport = { width: 300, height: 800 };
    const placement = placeGlance(rect(50, 100, 80, 20), CARD, viewport, {
      left: 0,
      right: 300,
    });
    expect(placement.left).toBe(12);
    expect(placement.width).toBe(276);
  });

  it("keeps a card too tall for either side within the viewport", () => {
    // A card taller than the room above and below its link fits on neither side.
    // Clamping keeps its top edge on screen, so the reader sees the opening of
    // the excerpt rather than a card hanging off the bottom.
    const tall = { width: 340, height: 700 };
    const placement = placeGlance(rect(200, 300, 80, 20), tall, VIEWPORT, FILLED);
    expect(placement.above).toBe(false);
    expect(placement.top).toBe(VIEWPORT.height - tall.height - 12); // 88
  });

  it("never places the card above the viewport margin", () => {
    // Taller than the whole viewport: the clamp's own range is inverted, and the
    // top margin wins so the card starts where it can be read.
    const huge = { width: 340, height: 900 };
    const placement = placeGlance(rect(200, 400, 80, 20), huge, VIEWPORT, FILLED);
    expect(placement.top).toBe(12);
  });
});
