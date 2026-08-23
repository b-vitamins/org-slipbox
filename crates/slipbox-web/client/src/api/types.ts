// Hand-maintained mirror of slipbox-core HTTP result types. Field names match
// serde output; keep changes in lockstep with the Rust definitions.

export type NodeKind = "file" | "heading";

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

export type AnchorRecord = NodeRecord;

export interface HealthInfo {
  status: string;
  root: string;
  version: string;
}

export interface StatusInfo {
  version: string;
  root: string;
  db: string;
  files_indexed: number;
  nodes_indexed: number;
  /** Addressable file nodes and id-bearing headings. */
  notes_indexed: number;
  links_indexed: number;
}

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

export interface BacklinkRecord {
  source_note: NodeRecord;
  source_anchor: AnchorRecord | null;
  row: number;
  col: number;
  preview: string;
  explanation: ExplorationExplanation;
}

export interface ForwardLinkRecord {
  destination_note: NodeRecord;
  row: number;
  col: number;
  preview: string;
  explanation: ExplorationExplanation;
}

export interface ReflinkRecord {
  source_anchor: AnchorRecord;
  row: number;
  col: number;
  preview: string;
  matched_reference: string;
  explanation: ExplorationExplanation;
}

export interface UnlinkedReferenceRecord {
  /** Owning note; absent on older daemons. */
  source_note?: NodeRecord;
  source_anchor: AnchorRecord;
  row: number;
  col: number;
  preview: string;
  matched_text: string;
  explanation: ExplorationExplanation;
}

export interface NotePlaceNeighbor {
  node_key: string;
  title: string;
}

export interface NotePlace {
  ordinal: number;
  total: number;
  earlier?: NotePlaceNeighbor;
  later?: NotePlaceNeighbor;
}

export interface NoteContext {
  note: NodeRecord;
  source: SourceSlice;
  node_start_line: number;
  node_line_count: number;
  /** Absent on older daemons. */
  place?: NotePlace;
  backlinks: BacklinkRecord[];
  forward_links: ForwardLinkRecord[];
  /** Unique related-note totals; absent on older daemons. */
  backlink_note_total?: number;
  forward_link_note_total?: number;
}

export interface SearchNodesResult {
  nodes: NodeRecord[];
}

export interface ContentSegment {
  text: string;
  matched: boolean;
}

export interface ContentSnippet {
  segments: ContentSegment[];
}

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

export type ExplorationLens =
  | "structure"
  | "refs"
  | "time"
  | "tasks"
  | "bridges"
  | "dormant"
  | "unresolved";

export type ExplorationSectionKind =
  | "backlinks"
  | "forward-links"
  | "reflinks"
  | "unlinked-references"
  | "time-neighbors"
  | "task-neighbors"
  | "bridge-candidates"
  | "dormant-notes"
  | "unresolved-tasks"
  | "weakly-integrated-notes";

export interface AnchorExplorationRecord {
  anchor: AnchorRecord;
  explanation: ExplorationExplanation;
}

/** Serde flattens each record beside its `kind` tag. */
export type ExplorationEntry =
  | ({ kind: "backlink" } & BacklinkRecord)
  | ({ kind: "forward-link" } & ForwardLinkRecord)
  | ({ kind: "reflink" } & ReflinkRecord)
  | ({ kind: "unlinked-reference" } & UnlinkedReferenceRecord)
  | ({ kind: "anchor" } & AnchorExplorationRecord);

export interface ExplorationSection {
  kind: ExplorationSectionKind;
  entries: ExplorationEntry[];
}

export interface ExploreResult {
  lens: ExplorationLens;
  sections: ExplorationSection[];
}

export interface GlossaryTermsResult {
  terms: NodeRecord[];
  total: number;
  has_more: boolean;
  next_position?: string | null;
}

export interface GlossaryTermResult {
  term: NodeRecord | null;
}

export type SearchNodesSort =
  | "relevance"
  | "title"
  | "file"
  | "file-mtime"
  | "backlink-count"
  | "forward-link-count";

export interface ApiErrorBody {
  error: {
    kind: string;
    message: string;
  };
}
