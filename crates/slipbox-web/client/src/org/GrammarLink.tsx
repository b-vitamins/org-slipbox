/*
 * An anchor routing gestures through the navigation grammar: hover or focus
 * glances, a click pins, an Alt-click goes, and a tap glances where the pointer
 * cannot hover. A browser gesture (new tab, non-primary button) is left to the
 * browser, whose destination is this target's `encodeStack` URL.
 */

import { type Component, type JSX } from "solid-js";

import { gestureCanHover } from "../dom/pointer.js";
import { encodeStack } from "../reading/stack.js";
import {
  referenceOf,
  useNavigation,
  type GlanceGesture,
  type LinkTarget,
} from "./navigation.jsx";

/** True for a click the browser should keep (new tab, non-primary button). */
function isBrowserGesture(event: MouseEvent): boolean {
  return (
    event.defaultPrevented ||
    event.metaKey ||
    event.ctrlKey ||
    event.button !== 0
  );
}

/**
 * True when this click is a press of the pointer rather than a keyboard
 * activation. Enter on a focused anchor synthesizes a click whose `detail` click
 * count is 0.
 */
function isPointerPress(event: MouseEvent): boolean {
  return event.detail > 0;
}

/** The real href a grammar link carries: the target opened as a fresh root. */
export function grammarHref(target: LinkTarget): string {
  return encodeStack([referenceOf(target)]);
}

export const GrammarLink: Component<{
  /** The internal target this link acts on. */
  target: LinkTarget;
  class?: string;
  classList?: Record<string, boolean | undefined>;
  children: JSX.Element;
}> = (props) => {
  const navigation = useNavigation();

  // Only a pointer event names its pointer; a click carries none, and the hover
  // and focus around a tap are synthesized. So the preceding pointer event is
  // recorded here and read by the handlers below. Not reactive: nothing renders
  // from it. It stays put until another pointer event replaces it, so a tap's own
  // synthesized hover and blur are still attributed to the tap.
  let pointerType: string | null = null;
  const record = (event: PointerEvent): void => {
    pointerType = event.pointerType || null;
  };

  const commit = (element: HTMLElement, alt: boolean): void => {
    navigation.glance(null);
    if (alt) {
      navigation.go(props.target);
    } else {
      navigation.pin(props.target);
    }
    // Drop focus so the just-followed link does not re-`glance` itself.
    element.blur();
  };

  const glance = (origin: HTMLElement, gesture: GlanceGesture): void => {
    navigation.glance({
      target: props.target,
      origin,
      gesture,
      pin: () => navigation.pin(props.target),
      go: () => navigation.go(props.target),
      dismiss: () => navigation.glance(null),
    });
  };

  const onClick = (event: MouseEvent): void => {
    if (isBrowserGesture(event)) {
      return;
    }
    event.preventDefault();
    const element = event.currentTarget as HTMLElement;
    // A press where the pointer cannot hover takes the hover's place: it raises
    // the preview, which carries `pin` and `go` as controls.
    if (isPointerPress(event) && !gestureCanHover(pointerType)) {
      glance(element, "touch");
      return;
    }
    commit(element, event.altKey);
  };

  const onHover = (event: MouseEvent): void => {
    if (gestureCanHover(pointerType)) {
      glance(event.currentTarget as HTMLElement, "pointer");
    }
  };

  const onFocus = (event: FocusEvent): void => {
    if (gestureCanHover(pointerType)) {
      glance(event.currentTarget as HTMLElement, "pointer");
    }
  };

  // Gated on hover capability: a mobile browser synthesizes hover and focus
  // around a tap, and dismissing on their departure would close the preview the
  // tap raised, on the way to its own controls.
  const dismiss = (): void => {
    if (gestureCanHover(pointerType)) {
      navigation.glance(null);
    }
  };

  return (
    <a
      class={props.class}
      classList={props.classList}
      href={grammarHref(props.target)}
      // These fire before the events the grammar routes on: pointerdown before
      // its click, pointerenter before the mouseenter and focus. Record only.
      onPointerDown={record}
      onPointerEnter={record}
      onClick={onClick}
      onMouseEnter={onHover}
      onMouseLeave={dismiss}
      onFocus={onFocus}
      onBlur={dismiss}
    >
      {props.children}
    </a>
  );
};
