/*
 * An anchor routing gestures through the navigation grammar: hover or focus
 * glances, Escape dismisses that glance, a click pins, an Alt-click goes, and a
 * tap glances where the pointer cannot hover. A browser gesture (new tab,
 * non-primary button) is left to the browser, whose destination is this target's
 * `encodeStack` URL.
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

  // Whether a card this link raised is standing, so a dismissal answers for a
  // card rather than for nothing. Not reactive: nothing renders from it. The card
  // is also dropped by things that are not gestures on this link - a scroll
  // carries the link out from under it - which is why the raise says how it is to
  // be told, rather than counting on its own request alone.
  let raised = false;
  const clear = (): void => {
    raised = false;
    navigation.glance(null);
  };

  const commit = (element: HTMLElement, alt: boolean): void => {
    clear();
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
      dismiss: clear,
      dropped: () => {
        raised = false;
      },
    });
    // Recorded after the handover, not before it: a card this link already had up
    // is dropped by the arrival of this one, and the drop is reported from inside
    // the call - so a raise that claimed the card first would be cleared by the
    // request it replaced.
    raised = true;
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
      glance(event.currentTarget as HTMLElement, "hover");
    }
  };

  // Tagged apart from the hover: the reader who arrived here by keyboard is not
  // looking at a cursor, so the card raised has to say so for itself.
  const onFocus = (event: FocusEvent): void => {
    if (gestureCanHover(pointerType)) {
      glance(event.currentTarget as HTMLElement, "focus");
    }
  };

  // The way out of a card raised by focus, where there is no pointer to move
  // away. It belongs to the link and not the document: this key is only a
  // dismissal while the reader is standing on the link the card came from, and
  // the link is where a keydown arrives. Nothing is remembered beyond the card
  // itself, so focus staying put is enough for the next hover or focus to raise it
  // again.
  //
  // Answered only where there is a card to answer for, and then consumed: Escape
  // is the conventional way out of anything transient, so a link that took it with
  // nothing up would eat the gesture a reader aimed past it, and one that let it
  // through after closing a card would spend it twice.
  const onKeyDown = (event: KeyboardEvent): void => {
    if (event.key !== "Escape" || !raised) {
      return;
    }
    clear();
    event.preventDefault();
  };

  // Gated on hover capability: a hoverless pointer synthesizes hover and focus
  // around a tap, and dismissing on their departure would close the preview the
  // tap raised, on the way to its own controls.
  const dismiss = (): void => {
    if (gestureCanHover(pointerType)) {
      clear();
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
      onKeyDown={onKeyDown}
    >
      {props.children}
    </a>
  );
};
