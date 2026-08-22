/*
 * TypeScript mirror of the slipbox-core result types the reading HTTP API
 * serves. Field names match serde's snake_case output and enum spellings match
 * their `rename_all` attributes. Hand-maintained, not generated: keep in
 * lockstep with slipbox-core (`nodes.rs`, `source.rs`, `relations.rs`,
 * `glossary.rs`, `diagnostics.rs`) and slipbox-web (`neighborhood`).
 */

export type NodeKind = "file" | "heading";

/** A node's identity and indexed metadata. Mirrors `NodeRecord`. */
export interface NodeRecord {
  node_key: string;
  explicit_id: string | null;
  file_path: string;
  title: string;
  outline_path: string;
  aliases: string[];
  tags: string[];
  refs: string[];
  todo_keyword: string | null;
  scheduled_for: string | null;
  deadline_for: string | null;
  closed_at: string | null;
  glossary: boolean;
  glossary_status: string | null;
  sr_due: string | null;
  sr_ease: string | null;
  sr_interval: string | null;
  sr_reps: string | null;
  sr_last: string | null;
  level: number;
  line: number;
  kind: NodeKind;
  file_mtime_ns: number;
  backlink_count: number;
  forward_link_count: number;
}

/** A link endpoint. Structurally identical to `NodeRecord`. */
export type AnchorRecord = NodeRecord;

/** Liveness and served-root identity. Mirrors `PingInfo`. */
export interface PingInfo {
  version: string;
  root: string;
  db: string;
}

/** Served root, database path, and derived-index counts. Mirrors `StatusInfo`. */
export interface StatusInfo {
  version: string;
  root: string;
  db: string;
  files_indexed: number;
  nodes_indexed: number;
  /**
   * Addressable notes: file nodes and headings carrying an explicit id. A
   * subset of `nodes_indexed`, which also counts plain headings.
   */
  notes_indexed: number;
  links_indexed: number;
}

/** A slice of a source file with truncation flags. Mirrors `SourceSlice`. */
export interface SourceSlice {
  file_path: string;
  start_line: number;
  line_count: number;
  total_lines: number;
  content: string;
  truncated_before: boolean;
  truncated_after: boolean;
}

export type PlanningField = "scheduled" | "deadline";

export interface PlanningRelationRecord {
  source_field: PlanningField;
  candidate_field: PlanningField;
  date: string;
}

export interface BridgeEvidenceRecord {
  node_key: string;
  explicit_id: string | null;
  title: string;
}

/**
 * Why a relation was surfaced. Serde tags this union on a `kind` field with
 * kebab-case variant names; both must match exactly. Mirrors
 * `ExplorationExplanation`.
 */
export type ExplorationExplanation =
  | { kind: "backlink" }
  | { kind: "forward-link" }
  | { kind: "shared-reference"; reference: string }
  | { kind: "unlinked-reference"; matched_text: string }
  | { kind: "time-neighbor"; relations: PlanningRelationRecord[] }
  | {
      kind: "task-neighbor";
      shared_todo_keyword: string | null;
      planning_relations: PlanningRelationRecord[];
    }
  | {
      kind: "bridge-candidate";
      references: string[];
      via_notes: BridgeEvidenceRecord[];
    }
  | {
      kind: "dormant-shared-reference";
      references: string[];
      modified_at_ns: number;
      via_notes: BridgeEvidenceRecord[];
    }
  | {
      kind: "unresolved-shared-reference";
      references: string[];
      todo_keyword: string;
    }
  | {
      kind: "weakly-integrated-shared-reference";
      references: string[];
      structural_link_count: number;
      via_notes: BridgeEvidenceRecord[];
    };

/** An incoming link. Mirrors `BacklinkRecord`. */
export interface BacklinkRecord {
  source_note: NodeRecord;
  source_anchor: AnchorRecord | null;
  row: number;
  col: number;
  preview: string;
  explanation: ExplorationExplanation;
}

/** An outgoing link. Mirrors `ForwardLinkRecord`. */
export interface ForwardLinkRecord {
  destination_note: NodeRecord;
  row: number;
  col: number;
  preview: string;
  explanation: ExplorationExplanation;
}

/** A link to a shared reference. Mirrors `ReflinkRecord`. */
export interface ReflinkRecord {
  source_anchor: AnchorRecord;
  row: number;
  col: number;
  preview: string;
  matched_reference: string;
  explanation: ExplorationExplanation;
}

/** An unlinked mention candidate. Mirrors `UnlinkedReferenceRecord`. */
export interface UnlinkedReferenceRecord {
  source_anchor: AnchorRecord;
  row: number;
  col: number;
  preview: string;
  matched_text: string;
  explanation: ExplorationExplanation;
}

/** A reading context: source slice plus immediate relations. Mirrors `NoteContextResult`. */
export interface NoteContext {
  note: NodeRecord;
  source: SourceSlice;
  node_start_line: number;
  node_line_count: number;
  backlinks: BacklinkRecord[];
  forward_links: ForwardLinkRecord[];
}

export interface SearchNodesResult {
  nodes: NodeRecord[];
}

/**
 * One run of a content-search excerpt, flagged with whether it matched the
 * query. The highlight is data, not markup, so note prose is never reinterpreted
 * by a renderer. Mirrors `ContentSegment`.
 */
export interface ContentSegment {
  text: string;
  matched: boolean;
}

/**
 * A highlighted excerpt of a note's body around a content-search match. The
 * segments concatenate, in order, to the excerpt text; an empty list means no
 * excerpt. Mirrors `ContentSnippet`.
 */
export interface ContentSnippet {
  segments: ContentSegment[];
}

/** One ranked content-search hit: the note, plus its excerpt. Mirrors `NodeContentHit`. */
export interface NodeContentHit {
  node: NodeRecord;
  snippet: ContentSnippet;
}

export interface SearchNodeContentResult {
  hits: NodeContentHit[];
}

export interface RandomNodeResult {
  node: NodeRecord | null;
}

export interface BacklinksResult {
  backlinks: BacklinkRecord[];
}

export interface ForwardLinksResult {
  forward_links: ForwardLinkRecord[];
}

export interface ReflinksResult {
  reflinks: ReflinkRecord[];
}

export interface UnlinkedReferencesResult {
  unlinked_references: UnlinkedReferenceRecord[];
}

/** Every glossary read returns the same list shape. Mirrors the `*Result { terms }` structs. */
export interface GlossaryTermsResult {
  terms: NodeRecord[];
}

/** One glossary term, or null when the key is unknown. Mirrors `GlossaryTermResult`. */
export interface GlossaryTermResult {
  term: NodeRecord | null;
}

/** How search results are ordered. Mirrors `SearchNodesSort` (kebab-case). */
export type SearchNodesSort =
  | "relevance"
  | "title"
  | "file"
  | "file-mtime"
  | "backlink-count"
  | "forward-link-count";

/** A directed edge in a neighborhood. Mirrors slipbox-web `EdgeKind`. */
export type EdgeKind = "forward";

export interface NeighborhoodNode {
  node: NodeRecord;
  distance: number;
}

export interface NeighborhoodEdge {
  source: string;
  target: string;
  kind: EdgeKind;
}

/** A hop-bounded local neighborhood. Mirrors slipbox-web `Neighborhood`. */
export interface Neighborhood {
  origin: string;
  hops: number;
  nodes: NeighborhoodNode[];
  edges: NeighborhoodEdge[];
  truncated: boolean;
}

/** The shared error envelope every failing response carries. */
export interface ApiErrorBody {
  error: {
    kind: string;
    message: string;
  };
}
