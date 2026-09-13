// Untrusted Org content may only create anchors for explicitly safe schemes.
const FOLLOWABLE_SCHEMES = new Set(["http", "https", "mailto"]);

// Match browser URL preprocessing before validating the scheme.
const STRIPPED_ANYWHERE = /[\t\n\r]/g;
const TRIMMED_ENDS = /^[\u0000-\u0020]+|[\u0000-\u0020]+$/g;
const SCHEME = /^([a-zA-Z][a-zA-Z0-9+.-]*):/;

function normalize(target: string): string {
  return target.replace(STRIPPED_ANYWHERE, "").replace(TRIMMED_ENDS, "");
}

/** Return a normalized safe href, or `null` for an unsupported target. */
export function followableHref(target: string): string | null {
  const normalized = normalize(target);
  const scheme = SCHEME.exec(normalized)?.[1];
  if (scheme === undefined) {
    return null;
  }
  return FOLLOWABLE_SCHEMES.has(scheme.toLowerCase()) ? normalized : null;
}

/*
 * Schemes a host may resolve an asset to. Narrower than a browser would accept:
 * `data:` is excluded because a document could otherwise be handed a whole HTML
 * page to navigate to, and `blob:` is admitted here but not to content, since
 * only the host can have created one.
 */
const RESOLVED_SCHEMES = new Set(["http", "https", "blob"]);

/*
 * A browser folds a backslash to a slash for every scheme reached from here, so
 * `\\host`, `/\host` and `https:\\host` all name an authority. The forms are
 * refused outright rather than rewritten, which keeps this check independent of
 * how far a given engine takes that folding.
 */
const AMBIGUOUS_SLASH = /\\/;

/**
 * Validate a URL a host resolved for an unfollowable target. A scheme-less URL
 * is relative to the document and allowed; `//host` and the backslash forms are
 * not, since a browser reads them as an authority off the document's own origin.
 * An admitted scheme carries whatever origin the host named.
 */
export function resolvedAssetHref(url: string): string | null {
  const normalized = normalize(url);
  if (
    normalized === "" ||
    normalized.startsWith("//") ||
    AMBIGUOUS_SLASH.test(normalized)
  ) {
    return null;
  }
  const scheme = SCHEME.exec(normalized)?.[1];
  if (scheme === undefined) {
    return normalized;
  }
  return RESOLVED_SCHEMES.has(scheme.toLowerCase()) ? normalized : null;
}
