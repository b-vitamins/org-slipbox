import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { afterEach, describe, expect, it, vi } from "vitest";

import {
  applyScheme,
  createColorScheme,
  decodeScheme,
  nextScheme,
  readStoredScheme,
  SCHEME_ATTRIBUTE,
  SCHEME_ORDER,
  SCHEME_STORAGE_KEY,
  storeScheme,
} from "./color-scheme.js";

/** Replace `localStorage` with an in-memory map seeded from `seed`. */
function stubStorage(seed: Record<string, string> = {}): Map<string, string> {
  const store = new Map(Object.entries(seed));
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => void store.set(key, value),
    removeItem: (key: string) => void store.delete(key),
  });
  return store;
}

/** Replace `localStorage` with one that throws, as a blocked-storage browser does. */
function stubBlockedStorage(): void {
  vi.stubGlobal("localStorage", {
    get getItem(): never {
      throw new Error("storage is blocked");
    },
    setItem: () => {
      throw new Error("storage is blocked");
    },
    removeItem: () => {
      throw new Error("storage is blocked");
    },
  });
}

describe("color scheme", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    document.documentElement.removeAttribute(SCHEME_ATTRIBUTE);
  });

  it("decodes only the two explicit schemes and treats anything else as the platform", () => {
    expect(decodeScheme("light")).toBe("light");
    expect(decodeScheme("dark")).toBe("dark");
    for (const raw of [null, undefined, "", "system", "sepia", "DARK"]) {
      expect(decodeScheme(raw)).toBe("system");
    }
  });

  it("cycles through every scheme and returns to where it started", () => {
    const walked = SCHEME_ORDER.map((_, step) =>
      SCHEME_ORDER.slice(0, step).reduce(nextScheme, "system"),
    );
    expect(walked).toEqual(["system", "light", "dark"]);
    expect(nextScheme("dark")).toBe("system");
  });

  it("applies an explicit scheme as an attribute and the platform as its absence", () => {
    applyScheme("dark");
    expect(document.documentElement.getAttribute(SCHEME_ATTRIBUTE)).toBe("dark");
    applyScheme("light");
    expect(document.documentElement.getAttribute(SCHEME_ATTRIBUTE)).toBe("light");
    applyScheme("system");
    expect(document.documentElement.hasAttribute(SCHEME_ATTRIBUTE)).toBe(false);
  });

  it("persists an explicit choice and forgets it again on returning to the platform", () => {
    const store = stubStorage();
    storeScheme("dark");
    expect(store.get(SCHEME_STORAGE_KEY)).toBe("dark");
    storeScheme("system");
    expect(store.has(SCHEME_STORAGE_KEY)).toBe(false);
  });

  it("seeds from the stored scheme and applies each choice as the reader cycles", () => {
    stubStorage({ [SCHEME_STORAGE_KEY]: "dark" });
    const scheme = createColorScheme();
    expect(scheme.scheme()).toBe("dark");

    scheme.cycle();
    expect(scheme.scheme()).toBe("system");
    expect(document.documentElement.hasAttribute(SCHEME_ATTRIBUTE)).toBe(false);
    expect(readStoredScheme()).toBe("system");

    scheme.cycle();
    expect(scheme.scheme()).toBe("light");
    expect(document.documentElement.getAttribute(SCHEME_ATTRIBUTE)).toBe("light");
    expect(readStoredScheme()).toBe("light");
  });

  it("still applies a scheme when storage is blocked", () => {
    stubBlockedStorage();
    const scheme = createColorScheme();
    // A browser that throws on the property must not cost the reader the surface.
    expect(scheme.scheme()).toBe("system");
    scheme.select("dark");
    expect(scheme.scheme()).toBe("dark");
    expect(document.documentElement.getAttribute(SCHEME_ATTRIBUTE)).toBe("dark");
  });

  it("keeps the pre-paint script in index.html on this module's key and attribute", () => {
    // The inline script is a second implementation of the stored read, running
    // before the bundle loads. It cannot import from here, so the contract it
    // shares — the storage key and the root attribute — is asserted instead.
    const html = readFileSync(resolve(process.cwd(), "index.html"), "utf8");
    expect(html).toContain(`getItem("${SCHEME_STORAGE_KEY}")`);
    expect(html).toContain(`setAttribute("${SCHEME_ATTRIBUTE}", scheme)`);
    expect(html).toContain('content="light dark"');
  });
});
