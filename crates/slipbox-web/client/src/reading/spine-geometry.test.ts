import { describe, expect, it } from "vitest";

import {
  columnOffset,
  columnState,
  gutterDepth,
  scrollTargetFor,
  verticalRevealTop,
  type SpineMetrics,
} from "./spine-geometry.js";

/** Reference metrics matching the design tokens (625px column, 40px sliver). */
const METRICS: SpineMetrics = {
  columnWidth: 625,
  sliver: 40,
  viewport: 1280,
  scrollWidth: 2000,
};

function spine(count: number, viewport: number): SpineMetrics {
  return { ...METRICS, viewport, scrollWidth: count * METRICS.columnWidth };
}

const VIEWPORTS = [700, 810, 900, 1024, 1200, 1280, 1440, 1920, 2560];

describe("gutterDepth", () => {
  it("holds as many slivers as fit beside one open column", () => {
    // 1280 - 625 = 655px of room, 16 slivers of 40px.
    expect(gutterDepth(METRICS)).toBe(16);
    expect(gutterDepth({ ...METRICS, viewport: 810 })).toBe(4);
  });

  it("holds nothing when a single column already fills the viewport", () => {
    expect(gutterDepth({ ...METRICS, viewport: 625 })).toBe(0);
    expect(gutterDepth({ ...METRICS, viewport: 400 })).toBe(0);
  });
});

describe("columnOffset", () => {
  it("climbs one sliver per column up to the ladder's depth", () => {
    expect(columnOffset(0, METRICS)).toBe(0);
    expect(columnOffset(3, METRICS)).toBe(120);
    expect(columnOffset(16, METRICS)).toBe(640);
  });

  it("stops climbing past the ladder's depth", () => {
    expect(columnOffset(17, METRICS)).toBe(640);
    expect(columnOffset(200, METRICS)).toBe(640);
    expect(columnOffset(200, METRICS)).toBeLessThan(METRICS.viewport);
  });
});

describe("columnState", () => {
  it("rests the first column at the origin", () => {
    expect(columnState(0, 3, 0, METRICS)).toBe("resting");
  });

  it("rests a column that is still in flow with room to be read", () => {
    expect(columnState(1, 3, 0, METRICS)).toBe("resting");
  });

  it("floats a column as an overlay once the stack pins behind it", () => {
    expect(columnState(1, 3, 100, METRICS)).toBe("overlay");
    expect(columnState(1, 3, 1089, METRICS)).toBe("overlay");
  });

  it("obscures a column the next column has covered to its sliver", () => {
    expect(columnState(1, 3, 1200, METRICS)).toBe("obscured");
  });

  it("obscures a far-right column that has not yet entered from the left", () => {
    expect(columnState(6, 8, 0, spine(8, 1280))).toBe("obscured");
  });

  it("keeps a readable column at every offset however deep the stack", () => {
    for (const viewport of VIEWPORTS) {
      for (const count of [19, 24, 31, 47, 80]) {
        const metrics = spine(count, viewport);
        const reach = metrics.scrollWidth - viewport;
        for (let step = 0; step <= 40; step += 1) {
          const scrollLeft = (reach * step) / 40;
          const states = Array.from({ length: count }, (_, index) =>
            columnState(index, count, scrollLeft, metrics),
          );
          expect(
            states.some((state) => state !== "obscured"),
            `viewport ${viewport}, ${count} columns, scrollLeft ${scrollLeft}`,
          ).toBe(true);
        }
      }
    }
  });
});

describe("verticalRevealTop", () => {
  it("brings a column below the fold to the top of the reader", () => {
    // Reader top 46 and unscrolled, column 900 down the viewport: 900 - 46 = 854.
    expect(verticalRevealTop({ top: 46, scrollTop: 0 }, 900)).toBe(854);
  });

  it("ignores the chrome above the reader", () => {
    const short = verticalRevealTop({ top: 20, scrollTop: 0 }, 20 + 700);
    const tall = verticalRevealTop({ top: 120, scrollTop: 0 }, 120 + 700);
    expect(short).toBe(700);
    expect(tall).toBe(700);
  });

  it("accounts for how far the reader is already scrolled", () => {
    // scrollTop 1200, column 400px above the reader's top edge: 1200 - 400 = 800.
    expect(verticalRevealTop({ top: 46, scrollTop: 1200 }, 46 - 400)).toBe(800);
  });

  it("never asks for a negative offset", () => {
    expect(verticalRevealTop({ top: 46, scrollTop: 0 }, 46)).toBe(0);
    expect(verticalRevealTop({ top: 46, scrollTop: 100 }, -900)).toBe(0);
  });
});

describe("scrollTargetFor", () => {
  it("returns null when the spine fits within the viewport", () => {
    const fits: SpineMetrics = { ...METRICS, scrollWidth: 600, viewport: 1280 };
    expect(scrollTargetFor(0, fits)).toBeNull();
  });

  it("centers a column when the spine overflows", () => {
    // index 1: 1*625 - (1280-625)/2 = 625 - 327.5 = 297.5, clamped to range.
    expect(scrollTargetFor(1, METRICS)).toBeCloseTo(297.5, 1);
  });

  it("clamps the target to the scrollable range", () => {
    // A far column cannot scroll past scrollWidth - viewport = 720.
    expect(scrollTargetFor(10, METRICS)).toBe(720);
  });

  it("leaves the column it reveals readable, at any depth", () => {
    for (const viewport of VIEWPORTS) {
      for (const count of [1, 2, 5, 17, 23, 29, 60]) {
        const metrics = spine(count, viewport);
        for (let index = 0; index < count; index += 1) {
          const target = scrollTargetFor(index, metrics) ?? 0;
          expect(
            columnState(index, count, target, metrics),
            `viewport ${viewport}, ${count} columns, revealing ${index}`,
          ).not.toBe("obscured");
        }
      }
    }
  });
});
