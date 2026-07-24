/*
 * Reactive document title: one owner for `document.title`, re-run when its
 * reactive label changes and restored to the base title on cleanup.
 */

import { createEffect, onCleanup } from "solid-js";

/** The product name: the tab suffix, and the title restored on cleanup. */
export const BASE_TITLE = "slipbox";

/**
 * Compose a tab title: a specific label ahead of the product name, or the
 * product name alone when the label is empty.
 */
export function formatTitle(label?: string | null): string {
  const trimmed = label?.trim();
  return trimmed ? `${trimmed} — ${BASE_TITLE}` : BASE_TITLE;
}

/**
 * Drive `document.title` from a reactive accessor for the lifetime of the
 * calling component. A falsy label shows the product name alone.
 */
export function useDocumentTitle(label: () => string | null | undefined): void {
  if (typeof document === "undefined") {
    return;
  }
  createEffect(() => {
    document.title = formatTitle(label());
  });
  onCleanup(() => {
    document.title = BASE_TITLE;
  });
}
