import { render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";

import { GrammarLink, grammarHref } from "./GrammarLink.jsx";
import {
  NavigationProvider,
  referenceOf,
  type GlanceRequest,
  type LinkTarget,
  type Navigation,
} from "./navigation.jsx";
import { decodeStack } from "../reading/stack.js";

function stubPointer(hoverless: boolean): void {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("hover: none") ? hoverless : false,
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

function spyNavigation(): {
  navigation: Navigation;
  glances: (GlanceRequest | null)[];
  pins: LinkTarget[];
  goes: LinkTarget[];
} {
  const glances: (GlanceRequest | null)[] = [];
  const pins: LinkTarget[] = [];
  const goes: LinkTarget[] = [];
  return {
    navigation: {
      glance: (request) => {
        const dropped = glances.at(-1);
        glances.push(request);
        if (dropped && dropped !== request) {
          dropped.dropped();
        }
      },
      pin: (target) => pins.push(target),
      go: (target) => goes.push(target),
    },
    glances,
    pins,
    goes,
  };
}

const TARGET: LinkTarget = { id: "abc-123", target: "id:abc-123" };

function renderLink(navigation: Navigation): HTMLAnchorElement {
  render(() => (
    <NavigationProvider navigation={navigation}>
      <GrammarLink target={TARGET}>the algorithm</GrammarLink>
    </NavigationProvider>
  ));
  return screen.getByRole("link", { name: "the algorithm" }) as HTMLAnchorElement;
}

/** A pointer press: a click carrying `detail: 1`, as a browser sends. */
function press(link: HTMLElement, init: MouseEventInit = {}): void {
  link.dispatchEvent(
    new MouseEvent("click", { bubbles: true, cancelable: true, detail: 1, ...init }),
  );
}

/**
 * jsdom has no `PointerEvent` constructor and its `MouseEvent` carries no
 * `pointerType`, so the field is defined onto one. It is the only part of a
 * pointer event the grammar reads.
 */
function pointerGesture(link: HTMLElement, type: string, pointerType: string): void {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true });
  Object.defineProperty(event, "pointerType", { value: pointerType });
  link.dispatchEvent(event);
}

describe("grammarHref", () => {
  it("emits the router's own URL for an id target, decodable back to it", () => {
    const target: LinkTarget = { id: "abc-123", target: "id:abc-123" };
    const href = grammarHref(target);

    expect(href).toBe("?note=id%3Aabc-123");
    expect(decodeStack(href)).toEqual([referenceOf(target)]);
  });

  it("emits the router's own URL for a raw-key target, decodable back to it", () => {
    const target: LinkTarget = { id: null, target: "notes/a.org::0" };
    const href = grammarHref(target);

    expect(decodeStack(href)).toEqual(["notes/a.org::0"]);
  });
});

describe("GrammarLink gestures", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  describe("where the pointer hovers", () => {
    it("glances on hover and commits on the press that follows", () => {
      stubPointer(false);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      link.dispatchEvent(new MouseEvent("mouseenter"));
      expect(spy.glances).toHaveLength(1);
      expect(spy.glances[0]?.gesture).toBe("hover");
      expect(spy.pins).toHaveLength(0);

      press(link);
      expect(spy.pins).toEqual([TARGET]);
      expect(spy.goes).toHaveLength(0);
    });

    it("tags a glance the keyboard raised apart from one the cursor raised", () => {
      stubPointer(false);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      link.dispatchEvent(new FocusEvent("focus"));
      expect(spy.glances[0]?.gesture).toBe("focus");

      link.dispatchEvent(new FocusEvent("blur"));
      link.dispatchEvent(new MouseEvent("mouseenter"));
      expect(spy.glances.at(-1)?.gesture).toBe("hover");
    });

    it("dismisses the card on Escape, without refusing to raise it again", () => {
      stubPointer(false);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      link.dispatchEvent(new FocusEvent("focus"));
      expect(spy.glances.at(-1)?.gesture).toBe("focus");

      link.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      );
      expect(spy.glances.at(-1)).toBeNull();

      link.dispatchEvent(new MouseEvent("mouseenter"));
      expect(spy.glances.at(-1)?.gesture).toBe("hover");
    });

    it("answers Escape for the card a second raise put up", () => {
      stubPointer(false);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      link.dispatchEvent(new FocusEvent("focus"));
      link.dispatchEvent(new MouseEvent("mouseenter"));
      expect(spy.glances.at(-1)?.gesture).toBe("hover");

      link.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      );
      expect(spy.glances.at(-1)).toBeNull();
    });

    it("leaves Escape to the surface where it has no card to dismiss", () => {
      stubPointer(false);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      const escape = (): boolean =>
        link.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "Escape",
            bubbles: true,
            cancelable: true,
          }),
        );

      expect(escape()).toBe(true);
      expect(spy.glances).toHaveLength(0);

      link.dispatchEvent(new FocusEvent("focus"));
      expect(escape()).toBe(false);
      expect(spy.glances.at(-1)).toBeNull();

      expect(escape()).toBe(true);
      expect(spy.glances).toHaveLength(2);
    });

    it("leaves every other key to the browser and the reader", () => {
      stubPointer(false);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      link.dispatchEvent(new FocusEvent("focus"));
      const raised = spy.glances.length;

      for (const key of ["Tab", "Enter", "ArrowDown", "Esc"]) {
        link.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
      }
      expect(spy.glances).toHaveLength(raised);
    });

    it("escalates to go on an alt-press", () => {
      stubPointer(false);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      press(link, { altKey: true });
      expect(spy.goes).toEqual([TARGET]);
      expect(spy.pins).toHaveLength(0);
    });
  });

  describe("where the pointer is a finger", () => {
    it("glances on a tap instead of pinning outright", () => {
      stubPointer(true);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      press(link);
      expect(spy.glances).toHaveLength(1);
      expect(spy.glances[0]?.gesture).toBe("touch");
      expect(spy.pins).toHaveLength(0);
      expect(spy.goes).toHaveLength(0);
    });

    it("hands the preview the verbs the gesture cannot reach", () => {
      stubPointer(true);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      press(link);
      const request = spy.glances[0]!;
      request.pin();
      expect(spy.pins).toEqual([TARGET]);
      request.go();
      expect(spy.goes).toEqual([TARGET]);
      request.dismiss();
      expect(spy.glances.at(-1)).toBeNull();
    });

    it("ignores the hover and focus a tap synthesizes around itself", () => {
      stubPointer(true);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      link.dispatchEvent(new MouseEvent("mouseenter"));
      link.dispatchEvent(new FocusEvent("focus"));
      expect(spy.glances).toHaveLength(0);

      press(link);
      link.dispatchEvent(new MouseEvent("mouseleave"));
      link.dispatchEvent(new FocusEvent("blur"));
      expect(spy.glances).toEqual([expect.objectContaining({ gesture: "touch" })]);
    });

    it("commits a keyboard activation rather than previewing it", () => {
      stubPointer(true);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      // A keyboard activation carries no click count.
      link.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      expect(spy.pins).toEqual([TARGET]);
      expect(spy.glances).toHaveLength(1);
      expect(spy.glances[0]).toBeNull();
    });
  });

  describe("where the device carries both pointers", () => {
    it("glances a finger's tap even where the primary pointer hovers", () => {
      stubPointer(false);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      pointerGesture(link, "pointerdown", "touch");
      press(link);
      expect(spy.glances).toEqual([expect.objectContaining({ gesture: "touch" })]);
      expect(spy.pins).toHaveLength(0);
      expect(spy.goes).toHaveLength(0);
    });

    it("commits a cursor's press even where the primary pointer is coarse", () => {
      stubPointer(true);
      const spy = spyNavigation();
      const link = renderLink(spy.navigation);

      pointerGesture(link, "pointerenter", "mouse");
      link.dispatchEvent(new MouseEvent("mouseenter"));
      expect(spy.glances).toEqual([expect.objectContaining({ gesture: "hover" })]);

      pointerGesture(link, "pointerdown", "mouse");
      press(link);
      expect(spy.pins).toEqual([TARGET]);
      expect(spy.glances.at(-1)).toBeNull();
    });
  });

  it("leaves a browser gesture to the browser", () => {
    stubPointer(true);
    const spy = spyNavigation();
    const link = renderLink(spy.navigation);

    // Solid delegates click to the document, so the link's own handler has run
    // by now; stopping the follow keeps jsdom from attempting a real navigation.
    const stop = (event: Event): void => event.preventDefault();
    document.addEventListener("click", stop);
    press(link, { metaKey: true });
    document.removeEventListener("click", stop);

    expect(spy.glances).toHaveLength(0);
    expect(spy.pins).toHaveLength(0);
    expect(spy.goes).toHaveLength(0);
  });
});
