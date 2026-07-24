/*
 * The served slipbox's display name, derived from the absolute root path the
 * daemon reports.
 */

/** The slipbox's display name: the last path segment of its root. */
export function slipboxName(root: string): string {
  const trimmed = root.replace(/\/+$/, "");
  const lastSlash = trimmed.lastIndexOf("/");
  const name = lastSlash === -1 ? trimmed : trimmed.slice(lastSlash + 1);
  // A root of only slashes, or an empty one, has no segment: fall back to `root`.
  return name === "" ? root : name;
}
