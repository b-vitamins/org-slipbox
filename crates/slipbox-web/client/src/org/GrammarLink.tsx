import { type Component, type JSX } from "solid-js";

import { gestureCanHover } from "../dom/pointer.js";
import { encodeStack } from "../reading/stack.js";
import {
  referenceOf,
  useNavigation,
  type GlanceGesture,
  type LinkTarget,
} from "./navigation.jsx";

export function isBrowserGesture(event: MouseEvent): boolean {
  return (
    event.defaultPrevented ||
    event.metaKey ||
    event.ctrlKey ||
    event.button !== 0
  );
}

/** Keyboard activation synthesizes a click with `detail === 0`. */
function isPointerPress(event: MouseEvent): boolean {
  return event.detail > 0;
}

export function grammarHref(target: LinkTarget): string {
  return encodeStack([referenceOf(target)]);
}

export const GrammarLink: Component<{
  target: LinkTarget;
  class?: string;
  classList?: Record<string, boolean | undefined>;
  children: JSX.Element;
}> = (props) => {
  const navigation = useNavigation();

  // Click events omit pointer type, so retain it from the preceding pointer event.
  let pointerType: string | null = null;
  const record = (event: PointerEvent): void => {
    pointerType = event.pointerType || null;
  };

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
    // Set after handover because replacing a card synchronously drops the old one.
    raised = true;
  };

  const onClick = (event: MouseEvent): void => {
    if (isBrowserGesture(event)) {
      return;
    }
    event.preventDefault();
    const element = event.currentTarget as HTMLElement;
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

  const onFocus = (event: FocusEvent): void => {
    if (gestureCanHover(pointerType)) {
      glance(event.currentTarget as HTMLElement, "focus");
    }
  };

  const onKeyDown = (event: KeyboardEvent): void => {
    if (event.key !== "Escape" || !raised) {
      return;
    }
    clear();
    event.preventDefault();
  };

  // Synthetic hover/blur must not dismiss a touch-raised preview.
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
