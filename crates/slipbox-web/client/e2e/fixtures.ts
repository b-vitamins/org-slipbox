/*
 * A stubbed JSON API for the end-to-end specs: an in-memory world served
 * through Playwright request interception, so a run needs no daemon, no
 * database, and no slipbox on disk. The envelopes reproduce the wire contract
 * in `src/api/types.ts`, serde's snake_case field names included, so the client
 * fetches, parses, and renders exactly as it does in production.
 */

import type { Page, Route } from "@playwright/test";

export interface FixtureNote {
  /** The slipbox key, `file:<path>` or `heading:<path>::<line>`. */
  key: string;
  /** Present only when the note is id-addressable; an `id:` link needs one. */
  id?: string;
  title: string;
  /** Raw Org source, which the reading column parses. */
  body: string;
  forwardLinks?: FixtureLink[];
  backlinks?: FixtureLink[];
  /** What the `bridges` lens ranks for this note, in the order it ranks it. */
  bridges?: FixtureBridge[];
  /** Lines in other notes naming this one, in the order the scan reports them. */
  mentions?: FixtureMention[];
  /** Present only on a marked glossary term; absence keeps a note out of it. */
  glossaryStatus?: "stub" | "confirmed";
  /** `YYYY-MM-DD`; a term with one set is what `/api/glossary/due` returns. */
  srDue?: string;
}

/** A note named by a relation, which the envelopes carry as a whole record. */
export interface FixtureEndpoint {
  key: string;
  id?: string;
  title: string;
}

export interface FixtureLink extends FixtureEndpoint {
  /** The linking line, verbatim Org; the client flattens it to a preview. */
  preview: string;
}

/** One bridge candidate: a note two hops away, and the notes it was reached by. */
export interface FixtureBridge extends FixtureEndpoint {
  via: FixtureEndpoint[];
}

/** One unlinked mention: the node whose line it is, that line, and the run matched. */
export interface FixtureMention {
  source: FixtureEndpoint;
  /** The mentioning line, verbatim Org. */
  line: string;
  /** The run of `line` the scan matched, which it reports the column of. */
  matched: string;
}

export interface FixtureWorld {
  notes: FixtureNote[];
  status?: Partial<StatusInfo>;
}

interface StatusInfo {
  version: string;
  root: string;
  db: string;
  files_indexed: number;
  nodes_indexed: number;
  notes_indexed: number;
  links_indexed: number;
}

/** A complete `NodeRecord`, every field the client parses filled in. */
function nodeRecord(note: FixtureNote): Record<string, unknown> {
  return {
    node_key: note.key,
    explicit_id: note.id ?? null,
    file_path: fileOf(note.key),
    title: note.title,
    outline_path: note.title,
    aliases: [],
    tags: [],
    refs: [],
    todo_keyword: null,
    scheduled_for: null,
    deadline_for: null,
    closed_at: null,
    glossary: note.glossaryStatus !== undefined,
    glossary_status: note.glossaryStatus ?? null,
    sr_due: note.srDue ?? null,
    sr_ease: null,
    sr_interval: null,
    sr_reps: null,
    sr_last: null,
    level: note.key.startsWith("heading:") ? 1 : 0,
    line: 1,
    kind: note.key.startsWith("heading:") ? "heading" : "file",
    file_mtime_ns: 0,
    backlink_count: (note.backlinks ?? []).length,
    forward_link_count: (note.forwardLinks ?? []).length,
  };
}

/** A relation endpoint's record. Its body is empty: only the link names it. */
function linkNode(link: FixtureEndpoint): Record<string, unknown> {
  return nodeRecord({ key: link.key, id: link.id, title: link.title, body: "" });
}

/** One `bridge-candidate` entry, its evidence notes named as the wire names them. */
function bridgeEntry(bridge: FixtureBridge): Record<string, unknown> {
  return {
    kind: "anchor",
    anchor: linkNode(bridge),
    explanation: {
      kind: "bridge-candidate",
      references: [],
      via_notes: bridge.via.map((note) => ({
        node_key: note.key,
        explicit_id: note.id ?? null,
        title: note.title,
      })),
    },
  };
}

/** One scanned mention, its column counted in characters from 1 as the scan does. */
function mentionRecord(mention: FixtureMention): Record<string, unknown> {
  return {
    // A fixture mention stands in its note's own body, so the note the scan
    // names and the node the line sits in are the one record.
    source_note: linkNode(mention.source),
    source_anchor: linkNode(mention.source),
    row: 1,
    col: mention.line.indexOf(mention.matched) + 1,
    preview: mention.line,
    matched_text: mention.matched,
    explanation: { kind: "unlinked-reference", matched_text: mention.matched },
  };
}

function fileOf(key: string): string {
  const withoutKind = key.replace(/^(file|heading):/, "");
  const path = withoutKind.split("::")[0] ?? withoutKind;
  return path.split(":")[0] ?? path;
}

/**
 * The `NoteContext` envelope. Every note is served whole and untruncated from
 * line 1, so `line_count` equals `total_lines`.
 */
function noteContext(note: FixtureNote): Record<string, unknown> {
  const lines = note.body.split("\n").length;
  return {
    note: nodeRecord(note),
    source: {
      file_path: fileOf(note.key),
      start_line: 1,
      line_count: lines,
      total_lines: lines,
      content: note.body,
      truncated_before: false,
      truncated_after: false,
    },
    node_start_line: 1,
    node_line_count: lines,
    backlinks: (note.backlinks ?? []).map((link) => ({
      source_note: linkNode(link),
      source_anchor: null,
      row: 1,
      col: 0,
      preview: link.preview,
      explanation: { kind: "backlink" },
    })),
    forward_links: (note.forwardLinks ?? []).map((link) => ({
      destination_note: linkNode(link),
      row: 1,
      col: 0,
      preview: link.preview,
      explanation: { kind: "forward-link" },
    })),
    // Totals count related notes, as the server counts them, where the arrays
    // above are link rows and may name one note twice.
    backlink_note_total: distinctNotes(note.backlinks),
    forward_link_note_total: distinctNotes(note.forwardLinks),
  };
}

function distinctNotes(links: FixtureLink[] | undefined): number {
  return new Set((links ?? []).map((link) => link.key)).size;
}

function isTerm(note: FixtureNote): boolean {
  return note.glossaryStatus !== undefined;
}

/** Substring of title-or-body, standing in for the server's FTS ranking. */
function matches(note: FixtureNote, query: string): boolean {
  const needle = query.toLowerCase();
  return (
    note.title.toLowerCase().includes(needle) ||
    note.body.toLowerCase().includes(needle)
  );
}

/**
 * Terms one glossary page holds. Far below the server's own ceiling, so a world of
 * a few dozen terms still spans pages, and a requested `limit` is clamped to it the
 * way the routes clamp theirs.
 */
const GLOSSARY_PAGE = 25;

/**
 * One page of `listed`, starting at the position `after` names. The position is an
 * index into the listing, opaque to the client and minted per page as the routes
 * mint theirs.
 */
function glossaryPage(
  listed: FixtureNote[],
  params: URLSearchParams,
): Record<string, unknown> {
  const from = Number(params.get("after") ?? 0);
  const limit = Math.min(
    Number(params.get("limit") ?? GLOSSARY_PAGE),
    GLOSSARY_PAGE,
  );
  const served = listed.slice(from, from + limit);
  const next = from + served.length;
  const more = next < listed.length;
  return {
    terms: served.map(nodeRecord),
    total: listed.length,
    has_more: more,
    next_position: more ? String(next) : null,
  };
}

/** Characters of context an excerpt carries on either side of the match. */
const EXCERPT_CONTEXT = 40;

/**
 * A `ContentSnippet` around the first occurrence of `query`. Like the server's,
 * the segments are slices of raw Org source cut at both ends, so a spec asserting
 * on one is asserting on markup the client has to render.
 */
function excerpt(note: FixtureNote, query: string): Record<string, unknown> {
  const at = note.body.toLowerCase().indexOf(query.toLowerCase());
  if (at === -1) {
    return { segments: [] };
  }
  const from = Math.max(0, at - EXCERPT_CONTEXT);
  const to = Math.min(note.body.length, at + query.length + EXCERPT_CONTEXT);
  return {
    segments: [
      { text: note.body.slice(from, at), matched: false },
      { text: note.body.slice(at, at + query.length), matched: true },
      { text: note.body.slice(at + query.length, to), matched: false },
    ].filter((segment) => segment.text.length > 0),
  };
}

function statusInfo(world: FixtureWorld): StatusInfo {
  return {
    version: "0.17.0",
    root: "/home/reader/slipbox",
    db: "/home/reader/slipbox/.slipbox/index.db",
    files_indexed: world.notes.length,
    nodes_indexed: world.notes.length,
    notes_indexed: world.notes.length,
    links_indexed: world.notes.reduce(
      (sum, note) => sum + (note.forwardLinks ?? []).length,
      0,
    ),
    ...world.status,
  };
}

async function json(route: Route, body: unknown): Promise<void> {
  await route.fulfill({
    status: 200,
    contentType: "application/json; charset=utf-8",
    body: JSON.stringify(body),
  });
}

/** The API's shared error envelope: `{ error: { kind, message } }`. */
async function apiError(
  route: Route,
  status: number,
  kind: string,
  message: string,
): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json; charset=utf-8",
    body: JSON.stringify({ error: { kind, message } }),
  });
}

/**
 * Intercept every `/api/` request for `page` and answer it from `world`. Must
 * be installed before the navigation that triggers the fetches. A key `world`
 * does not model answers with a real 404 envelope, so a spec can drive the
 * client's not-found path by naming a note that is absent.
 */
export async function mountApi(page: Page, world: FixtureWorld): Promise<void> {
  const byKey = new Map(world.notes.map((note) => [note.key, note]));
  const byId = new Map(
    world.notes.filter((note) => note.id).map((note) => [note.id as string, note]),
  );

  await page.route(/\/api\//, async (route) => {
    const url = new URL(route.request().url());
    const params = url.searchParams;

    switch (url.pathname) {
      case "/api/status":
        return json(route, statusInfo(world));

      case "/api/healthz":
        return json(route, { status: "ok" });

      case "/api/node": {
        const id = params.get("id");
        const key = params.get("key");
        const note = id ? byId.get(id) : key ? byKey.get(key) : undefined;
        return note
          ? json(route, nodeRecord(note))
          : apiError(route, 404, "not-found", "no note matched the given selector");
      }

      case "/api/note/context": {
        const note = byKey.get(params.get("key") ?? "");
        return note
          ? json(route, noteContext(note))
          : apiError(route, 404, "not-found", "no note for the given key");
      }

      // Only the `bridges` lens has a fixture, since it is the only one the
      // reading surface asks for; another lens is an unmodelled route.
      case "/api/explore": {
        const note = byKey.get(params.get("key") ?? "");
        if (!note || params.get("lens") !== "bridges") {
          return apiError(
            route,
            404,
            "not-found",
            `no fixture for lens ${params.get("lens") ?? ""}`,
          );
        }
        return json(route, {
          lens: "bridges",
          sections: [
            {
              kind: "bridge-candidates",
              entries: (note.bridges ?? []).map(bridgeEntry),
            },
          ],
        });
      }

      case "/api/unlinked-references": {
        const note = byKey.get(params.get("key") ?? "");
        return note
          ? json(route, {
              unlinked_references: (note.mentions ?? []).map(mentionRecord),
            })
          : apiError(route, 404, "not-found", "no note for the given key");
      }

      case "/api/search/nodes": {
        const q = (params.get("q") ?? "").toLowerCase();
        const nodes = world.notes
          .filter((note) => note.title.toLowerCase().includes(q))
          .map(nodeRecord);
        return json(route, { nodes });
      }

      // Unlike `/api/search/nodes`, this path matches bodies as well as titles
      // and carries an excerpt with the matched run flagged.
      case "/api/search/content": {
        const q = params.get("q") ?? "";
        const hits = world.notes
          .filter((note) => matches(note, q))
          .map((note) => ({ node: nodeRecord(note), snippet: excerpt(note, q) }));
        return json(route, { hits });
      }

      // The glossary routes serve the marked subset of the same notes, so a
      // world with nothing marked answers each with an empty term list.
      case "/api/glossary/terms":
        return json(route, glossaryPage(world.notes.filter(isTerm), params));

      // One term by key, however far down the listing it sits. An unmarked note
      // is no term, and the route answers for it the way it answers for a key
      // naming nothing at all.
      case "/api/glossary/term": {
        const note = byKey.get(params.get("key") ?? "");
        return note && isTerm(note)
          ? json(route, { term: nodeRecord(note) })
          : apiError(
              route,
              404,
              "not-found",
              "no glossary term found for the given key",
            );
      }

      // Ranked rather than ordered by a stored key, so this page carries its cut
      // and its total without a position to continue from.
      case "/api/glossary/search": {
        const q = params.get("q") ?? "";
        const matched = world.notes.filter(
          (note) => isTerm(note) && matches(note, q),
        );
        const served = matched.slice(0, GLOSSARY_PAGE);
        return json(route, {
          terms: served.map(nodeRecord),
          total: matched.length,
          has_more: served.length < matched.length,
        });
      }

      case "/api/glossary/due": {
        const q = (params.get("q") ?? "").toLowerCase();
        const due = world.notes.filter(
          (note) =>
            isTerm(note) &&
            note.srDue &&
            (q === "" || note.title.toLowerCase().includes(q)),
        );
        return json(
          route,
          glossaryPage(due, params),
        );
      }

      // Deterministic: always the first note in `world`, never a real choice.
      case "/api/random": {
        const first = world.notes[0];
        return json(route, { node: first ? nodeRecord(first) : null });
      }

      default:
        return apiError(route, 404, "not-found", `no reading route for ${url.pathname}`);
    }
  });
}

/** A body of `paragraphs` blank-line-separated paragraphs, one line each. */
export function tallBody(paragraphs = 80): string {
  return Array.from(
    { length: paragraphs },
    (_, i) =>
      `Paragraph ${i + 1}. This note is deliberately long so the reading ` +
      `column must scroll its own body rather than stretching the page.`,
  ).join("\n\n");
}
