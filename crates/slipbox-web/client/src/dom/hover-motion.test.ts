import { describe, expect, it } from "vitest";

import { createHoverMotion } from "./hover-motion.js";

describe("createHoverMotion", () => {
  it("counts the first crossing as a move, having seen none before it", () => {
    const motion = createHoverMotion();
    expect(motion.crossed({ x: 40, y: 120 })).toBe(true);
  });

  it("reads a crossing at a fresh position as the pointer arriving", () => {
    const motion = createHoverMotion();
    motion.moved({ x: 40, y: 120 });
    expect(motion.crossed({ x: 40, y: 148 })).toBe(true);
  });

  it("reads a crossing where the pointer already was as the box arriving", () => {
    const motion = createHoverMotion();
    motion.moved({ x: 40, y: 120 });
    expect(motion.crossed({ x: 40, y: 120 })).toBe(false);
  });

  it("tells one axis apart from the other", () => {
    const motion = createHoverMotion();
    motion.moved({ x: 40, y: 120 });
    expect(motion.crossed({ x: 41, y: 120 })).toBe(true);
    motion.moved({ x: 41, y: 120 });
    expect(motion.crossed({ x: 41, y: 121 })).toBe(true);
  });

  it("forgets where the pointer was once it leaves", () => {
    const motion = createHoverMotion();
    motion.moved({ x: 40, y: 120 });
    motion.left();
    expect(motion.crossed({ x: 40, y: 120 })).toBe(true);
  });

  it("forgets a position a crossing recorded, not only one a move did", () => {
    const motion = createHoverMotion();
    expect(motion.crossed({ x: 40, y: 120 })).toBe(true);
    motion.left();
    expect(motion.crossed({ x: 40, y: 120 })).toBe(true);
  });

  it("takes a crossing as the pointer's new position", () => {
    const motion = createHoverMotion();
    motion.moved({ x: 40, y: 120 });
    expect(motion.crossed({ x: 40, y: 160 })).toBe(true);
    expect(motion.crossed({ x: 40, y: 160 })).toBe(false);
  });
});
