/*
 * A source block: the code verbatim, a language tag, and a copy control. The
 * Clipboard API refuses a scripted write on an insecure origin, on a denied
 * permission, or where absent, so a refusal reports and selects the code
 * instead. The chrome is a live region, so the outcome is announced.
 */

import { Show, createSignal, onCleanup, type Component } from "solid-js";

/** How long the copy button reports an outcome before returning to rest. */
const COPIED_FEEDBACK_MS = 1200;

type CopyState = "resting" | "copied" | "failed";

/** The control's label and accessible name in each state. */
const COPY_LABELS: Record<CopyState, { text: string; description: string }> = {
  resting: { text: "Copy", description: "Copy code to clipboard" },
  copied: { text: "Copied", description: "Code copied to clipboard" },
  failed: {
    text: "Copy blocked",
    description: "The clipboard refused the copy; the code is selected to copy by hand",
  },
};

/**
 * Put `code` under the reader's own copy shortcut by selecting it.
 *
 * Guarded on both halves of the Selection API: a host lacking either has nothing
 * to offer here, and a failed copy must not raise on top of itself.
 */
function selectContents(code: HTMLElement): void {
  const selection = window.getSelection?.();
  if (!selection || typeof document.createRange !== "function") {
    return;
  }
  const range = document.createRange();
  range.selectNodeContents(code);
  selection.removeAllRanges();
  selection.addRange(range);
}

export const SourceBlock: Component<{ lang: string | null; code: string }> = (
  props,
) => {
  let code!: HTMLElement;
  const [state, setState] = createSignal<CopyState>("resting");

  // One revert in flight at a time, so a second press restarts the window rather
  // than being cut short by the first press's timer. `window.setTimeout` rather
  // than the bare global, whose return type in Node typings is not a number.
  let reverting: number | null = null;
  const report = (outcome: CopyState): void => {
    if (reverting !== null) {
      window.clearTimeout(reverting);
    }
    setState(outcome);
    reverting = window.setTimeout(() => {
      reverting = null;
      setState("resting");
    }, COPIED_FEEDBACK_MS);
  };
  onCleanup(() => {
    if (reverting !== null) {
      window.clearTimeout(reverting);
    }
  });

  const copy = async (): Promise<void> => {
    try {
      const clipboard = navigator.clipboard;
      if (!clipboard) {
        throw new Error("this browser exposes no clipboard");
      }
      await clipboard.writeText(props.code);
      report("copied");
    } catch {
      selectContents(code);
      report("failed");
    }
  };

  return (
    <figure class="org-src" data-lang={props.lang ?? undefined}>
      {/* A live region announces nothing it already held at mount, so the
          language tag is silent and only the changing label is announced. */}
      <figcaption class="org-src__chrome" aria-live="polite">
        <Show when={props.lang} fallback={<span class="org-src__lang" />}>
          {(lang) => <span class="org-src__lang">{lang()}</span>}
        </Show>
        <button
          type="button"
          class="org-src__copy"
          classList={{ "org-src__copy--failed": state() === "failed" }}
          onClick={() => void copy()}
          aria-label={COPY_LABELS[state()].description}
        >
          {COPY_LABELS[state()].text}
        </button>
      </figcaption>
      <pre class="org-src__code">
        <code ref={code}>{props.code}</code>
      </pre>
    </figure>
  );
};
