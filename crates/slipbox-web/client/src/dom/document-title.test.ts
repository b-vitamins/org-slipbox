import { createRoot, createSignal } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";

import {
  BASE_TITLE,
  formatTitle,
  useDocumentTitle,
} from "./document-title.js";

afterEach(() => {
  document.title = "";
});

describe("formatTitle", () => {
  it("puts a specific label ahead of the product name", () => {
    expect(formatTitle("Riemann integral")).toBe("Riemann integral — slipbox");
  });

  it("shows the product name alone for an empty, blank, or missing label", () => {
    expect(formatTitle("")).toBe(BASE_TITLE);
    expect(formatTitle("   ")).toBe(BASE_TITLE);
    expect(formatTitle(null)).toBe(BASE_TITLE);
    expect(formatTitle(undefined)).toBe(BASE_TITLE);
  });

  it("trims surrounding whitespace off the label", () => {
    expect(formatTitle("  spaced  ")).toBe("spaced — slipbox");
  });
});

describe("useDocumentTitle", () => {
  it("tracks a reactive label and restores the base title on cleanup", () => {
    const [label, setLabel] = createSignal<string | undefined>(undefined);

    const dispose = createRoot((dispose) => {
      useDocumentTitle(label);
      return dispose;
    });

    expect(document.title).toBe(BASE_TITLE);

    setLabel("A note");
    expect(document.title).toBe("A note — slipbox");

    setLabel(undefined);
    expect(document.title).toBe(BASE_TITLE);

    dispose();
    expect(document.title).toBe(BASE_TITLE);
  });
});
