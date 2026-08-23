/*
 * Typed client for the read-only reading HTTP API.
 *
 * Every method maps to one GET route. Slipbox keys embed `:` and `/`, so a key
 * is always an encoded query parameter and never a path segment, matching the
 * route contract on the server. Failing responses carry a shared
 * `{ error: { kind, message } }` envelope, surfaced here as `ApiError`.
 */

import type {
  ApiErrorBody,
  ExplorationLens,
  ExploreResult,
  GlossaryTermResult,
  GlossaryTermsResult,
  BacklinksResult,
  ForwardLinksResult,
  NodeRecord,
  NoteContext,
  PingInfo,
  RandomNodeResult,
  ReflinksResult,
  SearchNodeContentResult,
  SearchNodesResult,
  SearchNodesSort,
  StatusInfo,
  UnlinkedReferencesResult,
} from "./types.js";

/** A failed API call, carrying the server's status and error envelope. */
export class ApiError extends Error {
  readonly status: number;
  readonly kind: string;

  constructor(status: number, kind: string, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.kind = kind;
  }

  /** True when the request resolved to nothing (a 404 `not-found`). */
  get isNotFound(): boolean {
    return this.status === 404;
  }
}

type QueryValue = string | number | boolean | undefined;

function buildQuery(params: Record<string, QueryValue>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) {
      search.set(key, String(value));
    }
  }
  const query = search.toString();
  return query ? `?${query}` : "";
}

async function parseError(response: Response): Promise<ApiError> {
  let kind = "error";
  let message = `request failed with status ${response.status}`;
  try {
    const body = (await response.json()) as Partial<ApiErrorBody>;
    if (body.error) {
      kind = body.error.kind ?? kind;
      message = body.error.message ?? message;
    }
  } catch {
    // A non-JSON error body keeps the defaults.
  }
  return new ApiError(response.status, kind, message);
}

/** A read-only client bound to one base URL (empty string = same origin). */
export class ReadingClient {
  private readonly base: string;
  private readonly fetchImpl: typeof fetch | undefined;

  constructor(base = "", fetchImpl?: typeof fetch) {
    // Normalize away a trailing slash so path joins are unambiguous.
    this.base = base.replace(/\/$/, "");
    // Left unbound when absent, so the global `fetch` resolves at call time
    // rather than being snapshotted at construction.
    this.fetchImpl = fetchImpl;
  }

  private async get<T>(path: string): Promise<T> {
    const url = `${this.base}${path}`;
    const init: RequestInit = {
      method: "GET",
      headers: { accept: "application/json" },
    };
    // Invoked through `globalThis` so the platform `fetch` keeps a valid
    // receiver, and a late-installed override is honored on every call.
    const response = await (this.fetchImpl
      ? this.fetchImpl(url, init)
      : globalThis.fetch(url, init));
    if (!response.ok) {
      throw await parseError(response);
    }
    return (await response.json()) as T;
  }

  status(): Promise<StatusInfo> {
    return this.get<StatusInfo>("/api/status");
  }

  healthz(): Promise<{ status: string }> {
    return this.get<{ status: string }>("/api/healthz");
  }

  nodeByKey(key: string): Promise<NodeRecord> {
    return this.get<NodeRecord>(`/api/node${buildQuery({ key })}`);
  }

  nodeById(id: string): Promise<NodeRecord> {
    return this.get<NodeRecord>(`/api/node${buildQuery({ id })}`);
  }

  nodeByTitle(title: string): Promise<NodeRecord> {
    return this.get<NodeRecord>(`/api/node${buildQuery({ title })}`);
  }

  searchNodes(
    query: string,
    options: { limit?: number; sort?: SearchNodesSort } = {},
  ): Promise<SearchNodesResult> {
    return this.get<SearchNodesResult>(
      `/api/search/nodes${buildQuery({ q: query, limit: options.limit, sort: options.sort })}`,
    );
  }

  /** Ranked note-body search, with a highlighted excerpt per hit. */
  searchContent(
    query: string,
    options: { limit?: number } = {},
  ): Promise<SearchNodeContentResult> {
    return this.get<SearchNodeContentResult>(
      `/api/search/content${buildQuery({ q: query, limit: options.limit })}`,
    );
  }

  randomNode(): Promise<RandomNodeResult> {
    return this.get<RandomNodeResult>("/api/random");
  }

  noteContext(
    key: string,
    options: {
      before?: number;
      after?: number;
      maxLines?: number;
      relations?: number;
    } = {},
  ): Promise<NoteContext> {
    return this.get<NoteContext>(
      `/api/note/context${buildQuery({
        key,
        before: options.before,
        after: options.after,
        max_lines: options.maxLines,
        relations: options.relations,
      })}`,
    );
  }

  backlinks(
    key: string,
    options: { limit?: number; unique?: boolean } = {},
  ): Promise<BacklinksResult> {
    return this.get<BacklinksResult>(
      `/api/backlinks${buildQuery({ key, limit: options.limit, unique: options.unique })}`,
    );
  }

  forwardLinks(
    key: string,
    options: { limit?: number; unique?: boolean } = {},
  ): Promise<ForwardLinksResult> {
    return this.get<ForwardLinksResult>(
      `/api/forward-links${buildQuery({ key, limit: options.limit, unique: options.unique })}`,
    );
  }

  reflinks(key: string, options: { limit?: number } = {}): Promise<ReflinksResult> {
    return this.get<ReflinksResult>(
      `/api/reflinks${buildQuery({ key, limit: options.limit })}`,
    );
  }

  unlinkedReferences(
    key: string,
    options: { limit?: number; unique?: boolean } = {},
  ): Promise<UnlinkedReferencesResult> {
    return this.get<UnlinkedReferencesResult>(
      `/api/unlinked-references${buildQuery({
        key,
        limit: options.limit,
        unique: options.unique,
      })}`,
    );
  }

  /** Read one note through one exploration lens. The lens is required. */
  explore(
    key: string,
    lens: ExplorationLens,
    options: { limit?: number } = {},
  ): Promise<ExploreResult> {
    return this.get<ExploreResult>(
      `/api/explore${buildQuery({ key, lens, limit: options.limit })}`,
    );
  }

  glossaryTerms(options: { limit?: number } = {}): Promise<GlossaryTermsResult> {
    return this.get<GlossaryTermsResult>(
      `/api/glossary/terms${buildQuery({ limit: options.limit })}`,
    );
  }

  searchGlossary(
    query: string,
    options: { limit?: number } = {},
  ): Promise<GlossaryTermsResult> {
    return this.get<GlossaryTermsResult>(
      `/api/glossary/search${buildQuery({ q: query, limit: options.limit })}`,
    );
  }

  glossaryTerm(key: string): Promise<GlossaryTermResult> {
    return this.get<GlossaryTermResult>(
      `/api/glossary/term${buildQuery({ key })}`,
    );
  }

  glossaryDue(
    options: { today?: string; query?: string; limit?: number } = {},
  ): Promise<GlossaryTermsResult> {
    return this.get<GlossaryTermsResult>(
      `/api/glossary/due${buildQuery({
        today: options.today,
        q: options.query,
        limit: options.limit,
      })}`,
    );
  }
}

export const client = new ReadingClient();
