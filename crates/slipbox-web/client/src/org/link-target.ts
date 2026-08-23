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
