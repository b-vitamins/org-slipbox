/*
 * The web adapter between a rendered link target and this surface's URL grammar.
 */

import { referenceOf, type LinkTarget, type Navigation } from "../org/navigation.jsx";
import { encodeStack } from "./stack.js";

/** The reading URL that opens `target` as a fresh root. */
export function noteHref(target: LinkTarget): string {
  return encodeStack([referenceOf(target)]);
}

/**
 * Addresses without verbs, for prose rendered outside a reading column. A
 * modifier-click, a middle-click and a cold load still reach the note, while a
 * plain click has no column to open and does nothing.
 */
export const ADDRESSES_ONLY: Navigation = {
  href: noteHref,
  glance: () => {},
  pin: () => {},
  go: () => {},
};
