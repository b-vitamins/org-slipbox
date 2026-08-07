/*
 * Which external Org link targets the reading surface will follow.
 *
 * An Org body is not necessarily written by the reader: notes are imported,
 * clipped, and shared. A link target reaches the DOM as an `href`, so a scheme
 * that executes (`javascript:`) or carries its own document (`data:`) would run
 * in the reading origin, and that origin holds same-origin read access to every
 * note and glossary term the API serves. Read-only bars the write, not the read,
 * so the corpus itself is what such a target would be after.
 *
 * The rule is an allowlist rather than a denylist: only a scheme that navigates
 * earns an anchor, and every other target renders as inert text. That refuses
 * `file:` and relative targets too, which a reading surface reached over HTTP
 * cannot follow anyway.
 */

/** Schemes an external reading anchor may carry. */
const FOLLOWABLE_SCHEMES = new Set(["http", "https", "mailto"]);

/**
 * Tab and newline, which a browser strips from a URL wherever they appear
 * before parsing it. The scheme has to be read with them already gone, or a
 * target spelling "javascript:" with a tab inside it passes a check that the
 * navigation it performs would not.
 */
const STRIPPED_ANYWHERE = /[\t\n\r]/g;

/** Leading and trailing C0 controls and spaces, which a browser trims. */
const TRIMMED_ENDS = /^[\u0000-\u0020]+|[\u0000-\u0020]+$/g;

/** A URL scheme as RFC 3986 spells one. */
const SCHEME = /^([a-zA-Z][a-zA-Z0-9+.-]*):/;

/**
 * The target as a browser would read it, so the string checked below is the
 * string the navigation would use.
 */
function normalize(target: string): string {
  return target.replace(STRIPPED_ANYWHERE, "").replace(TRIMMED_ENDS, "");
}

/**
 * The href an external Org link may carry, or null when the surface will not
 * follow its target.
 *
 * The returned href is the normalized target rather than the raw one, so the
 * value that reaches the DOM is exactly the value that passed the allowlist.
 */
export function followableHref(target: string): string | null {
  const normalized = normalize(target);
  const scheme = SCHEME.exec(normalized)?.[1];
  if (scheme === undefined) {
    return null;
  }
  return FOLLOWABLE_SCHEMES.has(scheme.toLowerCase()) ? normalized : null;
}
