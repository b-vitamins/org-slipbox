// Distinguish deliberate pointer movement from layout moving a row under a
// stationary pointer.
export interface HoverPlace {
  readonly x: number;
  readonly y: number;
}

export interface HoverMotion {
  moved: (at: HoverPlace) => void;
  /** The first crossing after entering the surface counts as movement. */
  crossed: (at: HoverPlace) => boolean;
  left: () => void;
}

export function createHoverMotion(): HoverMotion {
  let last: HoverPlace | null = null;
  const moved = (at: HoverPlace): void => {
    last = at;
  };
  return {
    moved,
    crossed: (at: HoverPlace): boolean => {
      const followed = last === null || at.x !== last.x || at.y !== last.y;
      moved(at);
      return followed;
    },
    left: (): void => {
      last = null;
    },
  };
}
