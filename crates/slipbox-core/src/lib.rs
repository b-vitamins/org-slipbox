use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingInfo {
    pub version: String,
    pub root: String,
    pub db: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusInfo {
    pub version: String,
    pub root: String,
    pub db: String,
    pub files_indexed: u64,
    pub nodes_indexed: u64,
    pub links_indexed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedFilesResult {
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub file_path: String,
    pub title: String,
    pub mtime_ns: i64,
    pub node_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphTitleShortening {
    Truncate,
    Wrap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphParams {
    #[serde(default)]
    pub root_node_key: Option<String>,
    #[serde(default)]
    pub max_distance: Option<u32>,
    #[serde(default)]
    pub include_orphans: bool,
    #[serde(default)]
    pub hidden_link_types: Vec<String>,
    #[serde(default = "default_graph_max_title_length")]
    pub max_title_length: usize,
    #[serde(default)]
    pub shorten_titles: Option<GraphTitleShortening>,
    #[serde(default)]
    pub node_url_prefix: Option<String>,
}

impl GraphParams {
    #[must_use]
    pub fn normalized_hidden_link_types(&self) -> Vec<String> {
        let mut types = Vec::new();
        for link_type in &self.hidden_link_types {
            let normalized = link_type.trim().to_ascii_lowercase();
            if !normalized.is_empty() && !types.iter().any(|candidate| candidate == &normalized) {
                types.push(normalized);
            }
        }
        types
    }

    #[must_use]
    pub fn normalized_max_title_length(&self) -> usize {
        self.max_title_length.clamp(8, 500)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphResult {
    pub dot: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    File,
    Heading,
}

impl NodeKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Heading => "heading",
        }
    }
}

impl FromStr for NodeKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "file" => Ok(Self::File),
            "heading" => Ok(Self::Heading),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorRecord {
    pub node_key: String,
    pub explicit_id: Option<String>,
    pub file_path: String,
    pub title: String,
    pub outline_path: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub todo_keyword: Option<String>,
    #[serde(default)]
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub deadline_for: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
    #[serde(default)]
    pub file_mtime_ns: i64,
    #[serde(default)]
    pub backlink_count: u64,
    #[serde(default)]
    pub forward_link_count: u64,
}

impl AnchorRecord {
    #[must_use]
    pub fn is_note(&self) -> bool {
        matches!(self.kind, NodeKind::File) || self.explicit_id.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRecord {
    pub node_key: String,
    pub explicit_id: Option<String>,
    pub file_path: String,
    pub title: String,
    pub outline_path: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub todo_keyword: Option<String>,
    #[serde(default)]
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub deadline_for: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
    #[serde(default)]
    pub file_mtime_ns: i64,
    #[serde(default)]
    pub backlink_count: u64,
    #[serde(default)]
    pub forward_link_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewNodeRecord {
    pub node_key: String,
    pub explicit_id: Option<String>,
    pub file_path: String,
    pub title: String,
    pub outline_path: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub todo_keyword: Option<String>,
    #[serde(default)]
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub deadline_for: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedFile {
    pub file_path: String,
    pub title: String,
    pub mtime_ns: i64,
    pub nodes: Vec<IndexedNode>,
    pub links: Vec<IndexedLink>,
    pub occurrence_document: Option<IndexedOccurrenceDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedNode {
    pub node_key: String,
    pub explicit_id: Option<String>,
    pub file_path: String,
    pub title: String,
    pub outline_path: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub refs: Vec<String>,
    pub todo_keyword: Option<String>,
    pub scheduled_for: Option<String>,
    pub deadline_for: Option<String>,
    pub closed_at: Option<String>,
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
}

impl From<IndexedNode> for NodeRecord {
    fn from(node: IndexedNode) -> Self {
        Self {
            node_key: node.node_key,
            explicit_id: node.explicit_id,
            file_path: node.file_path,
            title: node.title,
            outline_path: node.outline_path,
            aliases: node.aliases,
            tags: node.tags,
            refs: node.refs,
            todo_keyword: node.todo_keyword,
            scheduled_for: node.scheduled_for,
            deadline_for: node.deadline_for,
            closed_at: node.closed_at,
            level: node.level,
            line: node.line,
            kind: node.kind,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        }
    }
}

impl TryFrom<AnchorRecord> for NodeRecord {
    type Error = AnchorRecord;

    fn try_from(anchor: AnchorRecord) -> Result<Self, Self::Error> {
        if anchor.is_note() {
            Ok(Self {
                node_key: anchor.node_key,
                explicit_id: anchor.explicit_id,
                file_path: anchor.file_path,
                title: anchor.title,
                outline_path: anchor.outline_path,
                aliases: anchor.aliases,
                tags: anchor.tags,
                refs: anchor.refs,
                todo_keyword: anchor.todo_keyword,
                scheduled_for: anchor.scheduled_for,
                deadline_for: anchor.deadline_for,
                closed_at: anchor.closed_at,
                level: anchor.level,
                line: anchor.line,
                kind: anchor.kind,
                file_mtime_ns: anchor.file_mtime_ns,
                backlink_count: anchor.backlink_count,
                forward_link_count: anchor.forward_link_count,
            })
        } else {
            Err(anchor)
        }
    }
}

impl From<NodeRecord> for AnchorRecord {
    fn from(node: NodeRecord) -> Self {
        Self {
            node_key: node.node_key,
            explicit_id: node.explicit_id,
            file_path: node.file_path,
            title: node.title,
            outline_path: node.outline_path,
            aliases: node.aliases,
            tags: node.tags,
            refs: node.refs,
            todo_keyword: node.todo_keyword,
            scheduled_for: node.scheduled_for,
            deadline_for: node.deadline_for,
            closed_at: node.closed_at,
            level: node.level,
            line: node.line,
            kind: node.kind,
            file_mtime_ns: node.file_mtime_ns,
            backlink_count: node.backlink_count,
            forward_link_count: node.forward_link_count,
        }
    }
}

impl From<IndexedNode> for PreviewNodeRecord {
    fn from(node: IndexedNode) -> Self {
        Self {
            node_key: node.node_key,
            explicit_id: node.explicit_id,
            file_path: node.file_path,
            title: node.title,
            outline_path: node.outline_path,
            aliases: node.aliases,
            tags: node.tags,
            refs: node.refs,
            todo_keyword: node.todo_keyword,
            scheduled_for: node.scheduled_for,
            deadline_for: node.deadline_for,
            closed_at: node.closed_at,
            level: node.level,
            line: node.line,
            kind: node.kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedLink {
    pub source_node_key: String,
    pub destination_explicit_id: String,
    pub line: u32,
    pub column: u32,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedOccurrenceDocument {
    pub file_path: String,
    pub search_text: String,
    pub line_rows: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexStats {
    pub files_indexed: u64,
    pub nodes_indexed: u64,
    pub links_indexed: u64,
}

impl IndexStats {
    pub fn accumulate(&mut self, other: &Self) {
        self.files_indexed += other.files_indexed;
        self.nodes_indexed += other.nodes_indexed;
        self.links_indexed += other.links_indexed;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchNodesSort {
    Relevance,
    Title,
    File,
    FileMtime,
    BacklinkCount,
    ForwardLinkCount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchNodesParams {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    #[serde(default)]
    pub sort: Option<SearchNodesSort>,
}

impl SearchNodesParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 200)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchNodesResult {
    pub nodes: Vec<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchFilesParams {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

impl SearchFilesParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 200)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchFilesResult {
    pub files: Vec<FileRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchOccurrencesParams {
    pub query: String,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
}

impl SearchOccurrencesParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchOccurrencesResult {
    pub occurrences: Vec<OccurrenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OccurrenceRecord {
    pub file_path: String,
    pub row: u32,
    pub col: u32,
    pub preview: String,
    pub matched_text: String,
    #[serde(default)]
    pub owning_anchor: Option<AnchorRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomNodeResult {
    pub node: Option<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchTagsParams {
    pub query: String,
    #[serde(default = "default_tag_limit")]
    pub limit: usize,
}

impl SearchTagsParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchTagsResult {
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFromIdParams {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFromKeyParams {
    pub node_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFromTitleOrAliasParams {
    pub title_or_alias: String,
    #[serde(default)]
    pub nocase: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAtPointParams {
    pub file_path: String,
    pub line: u32,
}

impl NodeAtPointParams {
    #[must_use]
    pub fn normalized_line(&self) -> u32 {
        self.line.max(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklinksParams {
    pub node_key: String,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
    #[serde(default)]
    pub unique: bool,
}

impl BacklinksParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklinksResult {
    pub backlinks: Vec<BacklinkRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeEvidenceRecord {
    pub node_key: String,
    #[serde(default)]
    pub explicit_id: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanningField {
    Scheduled,
    Deadline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanningRelationRecord {
    pub source_field: PlanningField,
    pub candidate_field: PlanningField,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExplorationExplanation {
    Backlink,
    ForwardLink,
    SharedReference {
        reference: String,
    },
    UnlinkedReference {
        matched_text: String,
    },
    TimeNeighbor {
        relations: Vec<PlanningRelationRecord>,
    },
    TaskNeighbor {
        #[serde(default)]
        shared_todo_keyword: Option<String>,
        #[serde(default)]
        planning_relations: Vec<PlanningRelationRecord>,
    },
    BridgeCandidate {
        references: Vec<String>,
        via_notes: Vec<BridgeEvidenceRecord>,
    },
    DormantSharedReference {
        references: Vec<String>,
        modified_at_ns: i64,
    },
    UnresolvedSharedReference {
        references: Vec<String>,
        todo_keyword: String,
    },
    WeaklyIntegratedSharedReference {
        references: Vec<String>,
        structural_link_count: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklinkRecord {
    pub source_note: NodeRecord,
    #[serde(default)]
    pub source_anchor: Option<AnchorRecord>,
    pub row: u32,
    pub col: u32,
    pub preview: String,
    pub explanation: ExplorationExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardLinksParams {
    pub node_key: String,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
    #[serde(default)]
    pub unique: bool,
}

impl ForwardLinksParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardLinksResult {
    pub forward_links: Vec<ForwardLinkRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardLinkRecord {
    pub destination_note: NodeRecord,
    pub row: u32,
    pub col: u32,
    pub preview: String,
    pub explanation: ExplorationExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflinksParams {
    pub node_key: String,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
}

impl ReflinksParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflinksResult {
    pub reflinks: Vec<ReflinkRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflinkRecord {
    pub source_anchor: AnchorRecord,
    pub row: u32,
    pub col: u32,
    pub preview: String,
    pub matched_reference: String,
    pub explanation: ExplorationExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlinkedReferencesParams {
    pub node_key: String,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
}

impl UnlinkedReferencesParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlinkedReferencesResult {
    pub unlinked_references: Vec<UnlinkedReferenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlinkedReferenceRecord {
    pub source_anchor: AnchorRecord,
    pub row: u32,
    pub col: u32,
    pub preview: String,
    pub matched_text: String,
    pub explanation: ExplorationExplanation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExplorationLens {
    Structure,
    Refs,
    Time,
    Tasks,
    Bridges,
    Dormant,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExplorationSectionKind {
    Backlinks,
    ForwardLinks,
    Reflinks,
    UnlinkedReferences,
    TimeNeighbors,
    TaskNeighbors,
    BridgeCandidates,
    DormantNotes,
    UnresolvedTasks,
    WeaklyIntegratedNotes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploreParams {
    pub node_key: String,
    pub lens: ExplorationLens,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
    #[serde(default)]
    pub unique: bool,
}

impl ExploreParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        if self.unique && self.lens != ExplorationLens::Structure {
            Some("explore unique is only supported for the structure lens".to_owned())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploreResult {
    pub lens: ExplorationLens,
    pub sections: Vec<ExplorationSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorationSection {
    pub kind: ExplorationSectionKind,
    pub entries: Vec<ExplorationEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorExplorationRecord {
    pub anchor: AnchorRecord,
    pub explanation: ExplorationExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExplorationEntry {
    Backlink {
        #[serde(flatten)]
        record: Box<BacklinkRecord>,
    },
    ForwardLink {
        #[serde(flatten)]
        record: Box<ForwardLinkRecord>,
    },
    Reflink {
        #[serde(flatten)]
        record: Box<ReflinkRecord>,
    },
    UnlinkedReference {
        #[serde(flatten)]
        record: Box<UnlinkedReferenceRecord>,
    },
    Anchor {
        #[serde(flatten)]
        record: Box<AnchorExplorationRecord>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompareNotesParams {
    pub left_node_key: String,
    pub right_node_key: String,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
}

impl CompareNotesParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoteComparisonSectionKind {
    SharedRefs,
    SharedPlanningDates,
    LeftOnlyRefs,
    RightOnlyRefs,
    SharedBacklinks,
    SharedForwardLinks,
    ContrastingTaskStates,
    PlanningTensions,
    IndirectConnectors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoteComparisonGroup {
    #[default]
    All,
    Overlap,
    Divergence,
    Tension,
}

impl NoteComparisonGroup {
    #[must_use]
    pub fn includes(self, kind: NoteComparisonSectionKind) -> bool {
        match self {
            Self::All => true,
            Self::Overlap => matches!(
                kind,
                NoteComparisonSectionKind::SharedRefs
                    | NoteComparisonSectionKind::SharedPlanningDates
                    | NoteComparisonSectionKind::SharedBacklinks
                    | NoteComparisonSectionKind::SharedForwardLinks
            ),
            Self::Divergence => matches!(
                kind,
                NoteComparisonSectionKind::LeftOnlyRefs | NoteComparisonSectionKind::RightOnlyRefs
            ),
            Self::Tension => matches!(
                kind,
                NoteComparisonSectionKind::ContrastingTaskStates
                    | NoteComparisonSectionKind::PlanningTensions
                    | NoteComparisonSectionKind::IndirectConnectors
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComparisonConnectorDirection {
    LeftToRight,
    RightToLeft,
    Bidirectional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NoteComparisonExplanation {
    SharedReference,
    SharedPlanningDate,
    LeftOnlyReference,
    RightOnlyReference,
    SharedBacklink,
    SharedForwardLink,
    ContrastingTaskState,
    PlanningTension,
    IndirectConnector {
        direction: ComparisonConnectorDirection,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonReferenceRecord {
    pub reference: String,
    pub explanation: NoteComparisonExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonNodeRecord {
    pub node: NodeRecord,
    pub explanation: NoteComparisonExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonPlanningRecord {
    pub date: String,
    pub left_field: PlanningField,
    pub right_field: PlanningField,
    pub explanation: NoteComparisonExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonTaskStateRecord {
    pub left_todo_keyword: String,
    pub right_todo_keyword: String,
    pub explanation: NoteComparisonExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NoteComparisonEntry {
    Reference {
        #[serde(flatten)]
        record: Box<ComparisonReferenceRecord>,
    },
    Node {
        #[serde(flatten)]
        record: Box<ComparisonNodeRecord>,
    },
    PlanningRelation {
        #[serde(flatten)]
        record: Box<ComparisonPlanningRecord>,
    },
    TaskState {
        #[serde(flatten)]
        record: Box<ComparisonTaskStateRecord>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteComparisonSection {
    pub kind: NoteComparisonSectionKind,
    pub entries: Vec<NoteComparisonEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteComparisonResult {
    pub left_note: NodeRecord,
    pub right_note: NodeRecord,
    pub sections: Vec<NoteComparisonSection>,
}

impl NoteComparisonResult {
    #[must_use]
    pub fn filtered_to_group(&self, group: NoteComparisonGroup) -> Self {
        if group == NoteComparisonGroup::All {
            return self.clone();
        }

        Self {
            left_note: self.left_note.clone(),
            right_note: self.right_note.clone(),
            sections: self
                .sections
                .iter()
                .filter(|section| group.includes(section.kind))
                .cloned()
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExplorationArtifactKind {
    LensView,
    Comparison,
    Trail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorationArtifactMetadata {
    pub artifact_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
}

impl ExplorationArtifactMetadata {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_artifact_id_field(&self.artifact_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorationArtifactSummary {
    #[serde(flatten)]
    pub metadata: ExplorationArtifactMetadata,
    pub kind: ExplorationArtifactKind,
}

impl From<&SavedExplorationArtifact> for ExplorationArtifactSummary {
    fn from(artifact: &SavedExplorationArtifact) -> Self {
        Self {
            metadata: artifact.metadata.clone(),
            kind: artifact.kind(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLensViewArtifact {
    pub root_node_key: String,
    pub current_node_key: String,
    pub lens: ExplorationLens,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub frozen_context: bool,
}

impl SavedLensViewArtifact {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }

    #[must_use]
    pub fn explore_params(&self) -> ExploreParams {
        ExploreParams {
            node_key: self.current_node_key.clone(),
            lens: self.lens,
            limit: self.normalized_limit(),
            unique: self.unique,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_required_text_field(&self.root_node_key, "root_node_key")
            .or_else(|| validate_required_text_field(&self.current_node_key, "current_node_key"))
            .or_else(|| {
                (!self.frozen_context && self.root_node_key.trim() != self.current_node_key.trim())
                    .then(|| {
                        "non-frozen lens-view artifacts must use current_node_key as root_node_key"
                            .to_owned()
                    })
            })
            .or_else(|| self.explore_params().validation_error())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedComparisonArtifact {
    pub root_node_key: String,
    pub left_node_key: String,
    pub right_node_key: String,
    pub active_lens: ExplorationLens,
    #[serde(default)]
    pub structure_unique: bool,
    #[serde(default)]
    pub comparison_group: NoteComparisonGroup,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
    #[serde(default)]
    pub frozen_context: bool,
}

impl SavedComparisonArtifact {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }

    #[must_use]
    pub fn compare_notes_params(&self) -> CompareNotesParams {
        CompareNotesParams {
            left_node_key: self.left_node_key.clone(),
            right_node_key: self.right_node_key.clone(),
            limit: self.normalized_limit(),
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_required_text_field(&self.root_node_key, "root_node_key")
            .or_else(|| validate_required_text_field(&self.left_node_key, "left_node_key"))
            .or_else(|| validate_required_text_field(&self.right_node_key, "right_node_key"))
            .or_else(|| {
                (!self.frozen_context && self.root_node_key.trim() != self.left_node_key.trim())
                    .then(|| {
                        "non-frozen comparison artifacts must use left_node_key as root_node_key"
                            .to_owned()
                    })
            })
            .or_else(|| {
                (self.left_node_key.trim() == self.right_node_key.trim())
                    .then(|| "left_node_key and right_node_key must differ".to_owned())
            })
            .or_else(|| {
                (self.structure_unique && self.active_lens != ExplorationLens::Structure).then(
                    || {
                        "comparison structure_unique is only supported for the structure lens"
                            .to_owned()
                    },
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SavedTrailStep {
    LensView {
        #[serde(flatten)]
        artifact: Box<SavedLensViewArtifact>,
    },
    Comparison {
        #[serde(flatten)]
        artifact: Box<SavedComparisonArtifact>,
    },
}

impl SavedTrailStep {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::LensView { artifact } => artifact.validation_error(),
            Self::Comparison { artifact } => artifact.validation_error(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTrailArtifact {
    pub steps: Vec<SavedTrailStep>,
    pub cursor: usize,
    #[serde(default)]
    pub detached_step: Option<Box<SavedTrailStep>>,
}

impl SavedTrailArtifact {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        if self.steps.is_empty() {
            return Some("trail artifacts must contain at least one step".to_owned());
        }
        if self.cursor >= self.steps.len() {
            return Some("trail cursor must point to an existing step".to_owned());
        }
        for (index, step) in self.steps.iter().enumerate() {
            if let Some(error) = step.validation_error() {
                return Some(format!("trail step {index} is invalid: {error}"));
            }
        }
        if let Some(step) = &self.detached_step
            && let Some(error) = step.validation_error()
        {
            return Some(format!("detached trail step is invalid: {error}"));
        }
        if let Some(step) = &self.detached_step
            && self.steps.iter().any(|existing| existing == step.as_ref())
        {
            return Some(
                "detached trail step must not duplicate any recorded trail step".to_owned(),
            );
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExplorationArtifactPayload {
    LensView {
        #[serde(flatten)]
        artifact: Box<SavedLensViewArtifact>,
    },
    Comparison {
        #[serde(flatten)]
        artifact: Box<SavedComparisonArtifact>,
    },
    Trail {
        #[serde(flatten)]
        artifact: Box<SavedTrailArtifact>,
    },
}

impl ExplorationArtifactPayload {
    #[must_use]
    pub fn kind(&self) -> ExplorationArtifactKind {
        match self {
            Self::LensView { .. } => ExplorationArtifactKind::LensView,
            Self::Comparison { .. } => ExplorationArtifactKind::Comparison,
            Self::Trail { .. } => ExplorationArtifactKind::Trail,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::LensView { artifact } => artifact.validation_error(),
            Self::Comparison { artifact } => artifact.validation_error(),
            Self::Trail { artifact } => artifact.validation_error(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedExplorationArtifact {
    #[serde(flatten)]
    pub metadata: ExplorationArtifactMetadata,
    #[serde(flatten)]
    pub payload: ExplorationArtifactPayload,
}

impl SavedExplorationArtifact {
    #[must_use]
    pub fn kind(&self) -> ExplorationArtifactKind {
        self.payload.kind()
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.metadata
            .validation_error()
            .or_else(|| self.payload.validation_error())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TrailReplayStepResult {
    LensView {
        artifact: Box<SavedLensViewArtifact>,
        root_note: Box<NodeRecord>,
        current_note: Box<NodeRecord>,
        result: Box<ExploreResult>,
    },
    Comparison {
        artifact: Box<SavedComparisonArtifact>,
        root_note: Box<NodeRecord>,
        result: Box<NoteComparisonResult>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrailReplayResult {
    pub steps: Vec<TrailReplayStepResult>,
    pub cursor: usize,
    #[serde(default)]
    pub detached_step: Option<Box<TrailReplayStepResult>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExecutedExplorationArtifactPayload {
    LensView {
        artifact: Box<SavedLensViewArtifact>,
        root_note: Box<NodeRecord>,
        current_note: Box<NodeRecord>,
        result: Box<ExploreResult>,
    },
    Comparison {
        artifact: Box<SavedComparisonArtifact>,
        root_note: Box<NodeRecord>,
        result: Box<NoteComparisonResult>,
    },
    Trail {
        artifact: Box<SavedTrailArtifact>,
        replay: Box<TrailReplayResult>,
    },
}

impl ExecutedExplorationArtifactPayload {
    #[must_use]
    pub fn kind(&self) -> ExplorationArtifactKind {
        match self {
            Self::LensView { .. } => ExplorationArtifactKind::LensView,
            Self::Comparison { .. } => ExplorationArtifactKind::Comparison,
            Self::Trail { .. } => ExplorationArtifactKind::Trail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutedExplorationArtifact {
    #[serde(flatten)]
    pub metadata: ExplorationArtifactMetadata,
    #[serde(flatten)]
    pub payload: ExecutedExplorationArtifactPayload,
}

impl ExecutedExplorationArtifact {
    #[must_use]
    pub fn kind(&self) -> ExplorationArtifactKind {
        self.payload.kind()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveExplorationArtifactParams {
    pub artifact: SavedExplorationArtifact,
}

impl SaveExplorationArtifactParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.artifact.validation_error()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveExplorationArtifactResult {
    pub artifact: ExplorationArtifactSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorationArtifactIdParams {
    pub artifact_id: String,
}

impl ExplorationArtifactIdParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_artifact_id_field(&self.artifact_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListExplorationArtifactsParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorationArtifactResult {
    pub artifact: SavedExplorationArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListExplorationArtifactsResult {
    pub artifacts: Vec<ExplorationArtifactSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteExplorationArtifactResult {
    pub artifact_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteExplorationArtifactResult {
    pub artifact: ExecutedExplorationArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgendaParams {
    pub start: String,
    pub end: String,
    #[serde(default = "default_agenda_limit")]
    pub limit: usize,
}

impl AgendaParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgendaResult {
    pub nodes: Vec<AnchorRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefRecord {
    pub reference: String,
    pub node: NodeRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRefsParams {
    pub query: String,
    #[serde(default = "default_ref_limit")]
    pub limit: usize,
}

impl SearchRefsParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 200)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRefsResult {
    pub refs: Vec<RefRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFromRefParams {
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureNodeParams {
    pub title: String,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
}

impl CaptureNodeParams {
    #[must_use]
    pub fn normalized_refs(&self) -> Vec<String> {
        let mut refs: Vec<String> = Vec::new();

        for reference in &self.refs {
            for normalized in normalize_reference(reference) {
                if normalized.is_empty()
                    || refs
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&normalized))
                {
                    continue;
                }
                refs.push(normalized);
            }
        }

        refs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureContentType {
    Plain,
    Entry,
    Item,
    Checkitem,
    TableLine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTemplateParams {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub node_key: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub outline_path: Vec<String>,
    pub capture_type: CaptureContentType,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub prepend: bool,
    #[serde(default)]
    pub empty_lines_before: u32,
    #[serde(default)]
    pub empty_lines_after: u32,
    #[serde(default)]
    pub table_line_pos: Option<String>,
}

impl CaptureTemplateParams {
    #[must_use]
    pub fn normalized_outline_path(&self) -> Vec<String> {
        normalize_string_values(&self.outline_path, false)
    }

    #[must_use]
    pub fn normalized_refs(&self) -> Vec<String> {
        let mut refs: Vec<String> = Vec::new();

        for reference in &self.refs {
            for normalized in normalize_reference(reference) {
                if normalized.is_empty()
                    || refs
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&normalized))
                {
                    continue;
                }
                refs.push(normalized);
            }
        }

        refs
    }

    #[must_use]
    pub fn normalized_empty_lines_before(&self) -> usize {
        self.empty_lines_before.min(8) as usize
    }

    #[must_use]
    pub fn normalized_empty_lines_after(&self) -> usize {
        self.empty_lines_after.min(8) as usize
    }

    #[must_use]
    pub fn normalized_table_line_pos(&self) -> Option<String> {
        self.table_line_pos.as_ref().and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTemplatePreviewParams {
    #[serde(flatten)]
    pub capture: CaptureTemplateParams,
    #[serde(default)]
    pub source_override: Option<String>,
    #[serde(default)]
    pub ensure_node_id: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTemplatePreviewResult {
    pub file_path: String,
    pub content: String,
    pub preview_node: PreviewNodeRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsureFileNodeParams {
    pub file_path: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppendHeadingParams {
    pub file_path: String,
    pub title: String,
    pub heading: String,
    #[serde(default = "default_heading_level")]
    pub level: u32,
}

impl AppendHeadingParams {
    #[must_use]
    pub fn normalized_level(&self) -> usize {
        self.level.clamp(1, 32) as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppendHeadingToNodeParams {
    pub node_key: String,
    pub heading: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppendHeadingAtOutlinePathParams {
    pub file_path: String,
    pub heading: String,
    #[serde(default)]
    pub outline_path: Vec<String>,
    #[serde(default)]
    pub head: Option<String>,
}

impl AppendHeadingAtOutlinePathParams {
    #[must_use]
    pub fn normalized_outline_path(&self) -> Vec<String> {
        normalize_string_values(&self.outline_path, false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsureNodeIdParams {
    pub node_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateNodeMetadataParams {
    pub node_key: String,
    #[serde(default)]
    pub aliases: Option<Vec<String>>,
    #[serde(default)]
    pub refs: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

impl UpdateNodeMetadataParams {
    #[must_use]
    pub fn normalized_aliases(&self) -> Option<Vec<String>> {
        self.aliases
            .as_ref()
            .map(|values| normalize_string_values(values, false))
    }

    #[must_use]
    pub fn normalized_refs(&self) -> Option<Vec<String>> {
        self.refs.as_ref().map(|values| {
            let mut refs = Vec::new();
            for value in values {
                for normalized in normalize_reference(value) {
                    if normalized.is_empty()
                        || refs
                            .iter()
                            .any(|existing: &String| existing.eq_ignore_ascii_case(&normalized))
                    {
                        continue;
                    }
                    refs.push(normalized);
                }
            }
            refs
        })
    }

    #[must_use]
    pub fn normalized_tags(&self) -> Option<Vec<String>> {
        self.tags
            .as_ref()
            .map(|values| normalize_string_values(values, false))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefileSubtreeParams {
    pub source_node_key: String,
    pub target_node_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefileRegionParams {
    pub file_path: String,
    pub start: u32,
    pub end: u32,
    pub target_node_key: String,
}

impl RefileRegionParams {
    #[must_use]
    pub fn normalized_range(&self) -> (usize, usize) {
        let start = self.start.max(1) as usize;
        let end = self.end.max(1) as usize;
        if start <= end {
            (start, end)
        } else {
            (end, start)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractSubtreeParams {
    pub source_node_key: String,
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewriteFileParams {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexFileParams {
    pub file_path: String,
}

const fn default_search_limit() -> usize {
    50
}

const fn default_backlink_limit() -> usize {
    200
}

const fn default_tag_limit() -> usize {
    200
}

const fn default_agenda_limit() -> usize {
    200
}

const fn default_ref_limit() -> usize {
    50
}

const fn default_graph_max_title_length() -> usize {
    100
}

const fn default_heading_level() -> u32 {
    1
}

#[must_use]
pub fn normalize_reference(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if let Some(inner) = trimmed
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    {
        let target = inner.split_once("][").map_or(inner, |(path, _)| path);
        return normalize_reference(target);
    }

    if let Some(key) = trimmed.strip_prefix('@') {
        return normalize_cite_keys(key);
    }

    if let Some(inner) = trimmed
        .strip_prefix("[cite:")
        .and_then(|value| value.strip_suffix(']'))
    {
        return extract_org_cite_keys(inner);
    }

    if let Some(path) = trimmed.strip_prefix("cite:") {
        return normalize_cite_keys(path);
    }

    vec![trimmed.to_owned()]
}

fn normalize_cite_keys(input: &str) -> Vec<String> {
    input
        .split([',', ';'])
        .filter_map(|part| {
            let key = part
                .trim()
                .trim_start_matches('@')
                .trim_start_matches("cite:")
                .trim();
            if key.is_empty() {
                None
            } else {
                Some(format!("@{key}"))
            }
        })
        .collect()
}

fn extract_org_cite_keys(input: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut current = String::new();
    let mut collecting = false;

    for character in input.chars() {
        if collecting {
            if is_cite_key_char(character) {
                current.push(character);
                continue;
            }
            if !current.is_empty() {
                refs.push(format!("@{current}"));
                current.clear();
            }
            collecting = false;
        }

        if character == '@' {
            collecting = true;
        }
    }

    if collecting && !current.is_empty() {
        refs.push(format!("@{current}"));
    }

    if refs.is_empty() {
        normalize_cite_keys(input)
    } else {
        refs
    }
}

fn is_cite_key_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':' | '.')
}

fn normalize_string_values(values: &[String], nocase: bool) -> Vec<String> {
    let mut normalized = Vec::new();

    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty()
            || normalized.iter().any(|existing: &String| {
                if nocase {
                    existing.eq_ignore_ascii_case(trimmed)
                } else {
                    existing == trimmed
                }
            })
        {
            continue;
        }

        normalized.push(trimmed.to_owned());
    }

    normalized
}

fn validate_required_text_field(value: &str, field: &str) -> Option<String> {
    value
        .trim()
        .is_empty()
        .then(|| format!("{field} must not be empty"))
}

fn validate_artifact_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "artifact_id").or_else(|| {
        (value.trim() != value)
            .then(|| "artifact_id must not have leading or trailing whitespace".to_owned())
    })
}

fn validate_optional_text_field(value: Option<&str>, field: &str) -> Option<String> {
    value.and_then(|text| validate_required_text_field(text, field))
}

#[cfg(test)]
mod tests {
    use super::{
        BacklinkRecord, BridgeEvidenceRecord, CaptureNodeParams, CaptureTemplatePreviewResult,
        CompareNotesParams, ComparisonConnectorDirection, ComparisonPlanningRecord,
        ComparisonReferenceRecord, ComparisonTaskStateRecord, DeleteExplorationArtifactResult,
        ExecuteExplorationArtifactResult, ExecutedExplorationArtifact,
        ExecutedExplorationArtifactPayload, ExplorationArtifactIdParams, ExplorationArtifactKind,
        ExplorationArtifactMetadata, ExplorationArtifactPayload, ExplorationArtifactResult,
        ExplorationArtifactSummary, ExplorationEntry, ExplorationExplanation, ExplorationLens,
        ExplorationSection, ExplorationSectionKind, ExploreParams, ExploreResult,
        ListExplorationArtifactsResult, NodeFromKeyParams, NodeFromTitleOrAliasParams, NodeKind,
        NodeRecord, NoteComparisonEntry, NoteComparisonExplanation, NoteComparisonGroup,
        NoteComparisonResult, NoteComparisonSection, NoteComparisonSectionKind, PlanningField,
        PlanningRelationRecord, PreviewNodeRecord, SaveExplorationArtifactParams,
        SaveExplorationArtifactResult, SavedComparisonArtifact, SavedExplorationArtifact,
        SavedLensViewArtifact, SavedTrailArtifact, SavedTrailStep, SearchNodesParams,
        SearchNodesSort, TrailReplayResult, TrailReplayStepResult, UnlinkedReferencesParams,
        UpdateNodeMetadataParams, normalize_reference,
    };
    use serde_json::json;

    #[test]
    fn normalizes_common_reference_forms() {
        assert_eq!(normalize_reference("@thrun2005"), vec!["@thrun2005"]);
        assert_eq!(normalize_reference("cite:thrun2005"), vec!["@thrun2005"]);
        assert_eq!(
            normalize_reference("[cite:@thrun2005; @smith2024]"),
            vec!["@thrun2005", "@smith2024"]
        );
        assert_eq!(
            normalize_reference("[[https://example.test/path][Example]]"),
            vec!["https://example.test/path"]
        );
    }

    #[test]
    fn capture_params_normalize_and_deduplicate_refs() {
        let params = CaptureNodeParams {
            title: "Note".to_owned(),
            file_path: None,
            head: None,
            refs: vec![
                "cite:smith2024".to_owned(),
                "@smith2024".to_owned(),
                "https://example.test".to_owned(),
            ],
        };

        assert_eq!(
            params.normalized_refs(),
            vec!["@smith2024".to_owned(), "https://example.test".to_owned()]
        );
    }

    #[test]
    fn metadata_params_normalize_fields() {
        let params = UpdateNodeMetadataParams {
            node_key: "heading:note.org:3".to_owned(),
            aliases: Some(vec![
                " Bruce ".to_owned(),
                "Bruce".to_owned(),
                String::new(),
            ]),
            refs: Some(vec!["cite:smith2024".to_owned(), "@smith2024".to_owned()]),
            tags: Some(vec![
                "alpha".to_owned(),
                " alpha ".to_owned(),
                "beta".to_owned(),
            ]),
        };

        assert_eq!(params.normalized_aliases(), Some(vec!["Bruce".to_owned()]));
        assert_eq!(
            params.normalized_refs(),
            Some(vec!["@smith2024".to_owned()])
        );
        assert_eq!(
            params.normalized_tags(),
            Some(vec!["alpha".to_owned(), "beta".to_owned()])
        );
    }

    #[test]
    fn node_record_serialization_includes_metadata_fields() {
        let node = NodeRecord {
            node_key: "heading:note.org:3".to_owned(),
            explicit_id: Some("note-id".to_owned()),
            file_path: "note.org".to_owned(),
            title: "Note".to_owned(),
            outline_path: "Parent".to_owned(),
            aliases: vec!["Alias".to_owned()],
            tags: vec!["tag".to_owned()],
            refs: vec!["@smith2024".to_owned()],
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            level: 1,
            line: 3,
            kind: NodeKind::Heading,
            file_mtime_ns: 123,
            backlink_count: 2,
            forward_link_count: 4,
        };

        assert_eq!(
            serde_json::to_value(node).expect("node record should serialize"),
            json!({
                "node_key": "heading:note.org:3",
                "explicit_id": "note-id",
                "file_path": "note.org",
                "title": "Note",
                "outline_path": "Parent",
                "aliases": ["Alias"],
                "tags": ["tag"],
                "refs": ["@smith2024"],
                "todo_keyword": null,
                "scheduled_for": null,
                "deadline_for": null,
                "closed_at": null,
                "level": 1,
                "line": 3,
                "kind": "heading",
                "file_mtime_ns": 123,
                "backlink_count": 2,
                "forward_link_count": 4
            })
        );
    }

    #[test]
    fn preview_node_serialization_omits_indexed_metadata_fields() {
        let preview = PreviewNodeRecord {
            node_key: "heading:note.org:3".to_owned(),
            explicit_id: Some("note-id".to_owned()),
            file_path: "note.org".to_owned(),
            title: "Note".to_owned(),
            outline_path: "Parent".to_owned(),
            aliases: vec!["Alias".to_owned()],
            tags: vec!["tag".to_owned()],
            refs: vec!["@smith2024".to_owned()],
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            level: 1,
            line: 3,
            kind: NodeKind::Heading,
        };

        assert_eq!(
            serde_json::to_value(preview).expect("preview node should serialize"),
            json!({
                "node_key": "heading:note.org:3",
                "explicit_id": "note-id",
                "file_path": "note.org",
                "title": "Note",
                "outline_path": "Parent",
                "aliases": ["Alias"],
                "tags": ["tag"],
                "refs": ["@smith2024"],
                "todo_keyword": null,
                "scheduled_for": null,
                "deadline_for": null,
                "closed_at": null,
                "level": 1,
                "line": 3,
                "kind": "heading"
            })
        );
    }

    #[test]
    fn capture_template_preview_result_serializes_preview_node_field() {
        let result = CaptureTemplatePreviewResult {
            file_path: "note.org".to_owned(),
            content: "* Note\n".to_owned(),
            preview_node: PreviewNodeRecord {
                node_key: "heading:note.org:1".to_owned(),
                explicit_id: None,
                file_path: "note.org".to_owned(),
                title: "Note".to_owned(),
                outline_path: "Note".to_owned(),
                aliases: Vec::new(),
                tags: Vec::new(),
                refs: Vec::new(),
                todo_keyword: None,
                scheduled_for: None,
                deadline_for: None,
                closed_at: None,
                level: 1,
                line: 1,
                kind: NodeKind::Heading,
            },
        };

        assert_eq!(
            serde_json::to_value(result).expect("preview result should serialize"),
            json!({
                "file_path": "note.org",
                "content": "* Note\n",
                "preview_node": {
                    "node_key": "heading:note.org:1",
                    "explicit_id": null,
                    "file_path": "note.org",
                    "title": "Note",
                    "outline_path": "Note",
                    "aliases": [],
                    "tags": [],
                    "refs": [],
                    "todo_keyword": null,
                    "scheduled_for": null,
                    "deadline_for": null,
                    "closed_at": null,
                    "level": 1,
                    "line": 1,
                    "kind": "heading"
                }
            })
        );
    }

    #[test]
    fn exploration_explanation_serializes_with_tagged_kinds() {
        assert_eq!(
            serde_json::to_value(ExplorationExplanation::Backlink)
                .expect("backlink explanation should serialize"),
            json!({ "kind": "backlink" })
        );

        assert_eq!(
            serde_json::to_value(ExplorationExplanation::SharedReference {
                reference: "cite:smith2024".to_owned(),
            })
            .expect("shared reference explanation should serialize"),
            json!({
                "kind": "shared-reference",
                "reference": "cite:smith2024"
            })
        );

        assert_eq!(
            serde_json::to_value(ExplorationExplanation::UnlinkedReference {
                matched_text: "Project Atlas".to_owned(),
            })
            .expect("unlinked reference explanation should serialize"),
            json!({
                "kind": "unlinked-reference",
                "matched_text": "Project Atlas"
            })
        );

        assert_eq!(
            serde_json::to_value(ExplorationExplanation::TimeNeighbor {
                relations: vec![
                    PlanningRelationRecord {
                        source_field: PlanningField::Scheduled,
                        candidate_field: PlanningField::Scheduled,
                        date: "2026-05-01".to_owned(),
                    },
                    PlanningRelationRecord {
                        source_field: PlanningField::Deadline,
                        candidate_field: PlanningField::Scheduled,
                        date: "2026-05-03".to_owned(),
                    },
                ],
            })
            .expect("time-neighbor explanation should serialize"),
            json!({
                "kind": "time-neighbor",
                "relations": [
                    {
                        "source_field": "scheduled",
                        "candidate_field": "scheduled",
                        "date": "2026-05-01"
                    },
                    {
                        "source_field": "deadline",
                        "candidate_field": "scheduled",
                        "date": "2026-05-03"
                    }
                ]
            })
        );

        assert_eq!(
            serde_json::to_value(ExplorationExplanation::TaskNeighbor {
                shared_todo_keyword: Some("TODO".to_owned()),
                planning_relations: vec![PlanningRelationRecord {
                    source_field: PlanningField::Scheduled,
                    candidate_field: PlanningField::Deadline,
                    date: "2026-05-01".to_owned(),
                }],
            })
            .expect("task-neighbor explanation should serialize"),
            json!({
                "kind": "task-neighbor",
                "shared_todo_keyword": "TODO",
                "planning_relations": [
                    {
                        "source_field": "scheduled",
                        "candidate_field": "deadline",
                        "date": "2026-05-01"
                    }
                ]
            })
        );

        assert_eq!(
            serde_json::to_value(ExplorationExplanation::BridgeCandidate {
                references: vec!["@shared2024".to_owned(), "@shared2025".to_owned()],
                via_notes: vec![
                    BridgeEvidenceRecord {
                        node_key: "heading:neighbor.org:3".to_owned(),
                        explicit_id: Some("neighbor-id".to_owned()),
                        title: "Neighbor".to_owned(),
                    },
                    BridgeEvidenceRecord {
                        node_key: "heading:support.org:7".to_owned(),
                        explicit_id: Some("support-id".to_owned()),
                        title: "Support".to_owned(),
                    },
                ],
            })
            .expect("bridge explanation should serialize"),
            json!({
                "kind": "bridge-candidate",
                "references": ["@shared2024", "@shared2025"],
                "via_notes": [
                    {
                        "node_key": "heading:neighbor.org:3",
                        "explicit_id": "neighbor-id",
                        "title": "Neighbor"
                    },
                    {
                        "node_key": "heading:support.org:7",
                        "explicit_id": "support-id",
                        "title": "Support"
                    }
                ]
            })
        );

        assert_eq!(
            serde_json::to_value(ExplorationExplanation::DormantSharedReference {
                references: vec!["@shared2024".to_owned(), "@shared2025".to_owned()],
                modified_at_ns: 42,
            })
            .expect("dormant explanation should serialize"),
            json!({
                "kind": "dormant-shared-reference",
                "references": ["@shared2024", "@shared2025"],
                "modified_at_ns": 42
            })
        );

        assert_eq!(
            serde_json::to_value(ExplorationExplanation::UnresolvedSharedReference {
                references: vec!["@shared2024".to_owned(), "@shared2025".to_owned()],
                todo_keyword: "TODO".to_owned(),
            })
            .expect("unresolved explanation should serialize"),
            json!({
                "kind": "unresolved-shared-reference",
                "references": ["@shared2024", "@shared2025"],
                "todo_keyword": "TODO"
            })
        );

        assert_eq!(
            serde_json::to_value(ExplorationExplanation::WeaklyIntegratedSharedReference {
                references: vec!["@shared2024".to_owned(), "@shared2025".to_owned()],
                structural_link_count: 1,
            })
            .expect("weak integration explanation should serialize"),
            json!({
                "kind": "weakly-integrated-shared-reference",
                "references": ["@shared2024", "@shared2025"],
                "structural_link_count": 1
            })
        );
    }

    #[test]
    fn explore_params_round_trip_and_validate() {
        let params: ExploreParams = serde_json::from_value(json!({
            "node_key": "heading:alpha.org:3",
            "lens": "structure",
            "limit": 0,
            "unique": true
        }))
        .expect("explore params should deserialize");

        assert_eq!(params.node_key, "heading:alpha.org:3");
        assert_eq!(params.lens, ExplorationLens::Structure);
        assert_eq!(params.normalized_limit(), 1);
        assert_eq!(params.validation_error(), None);

        assert_eq!(
            serde_json::to_value(&params).expect("explore params should serialize"),
            json!({
                "node_key": "heading:alpha.org:3",
                "lens": "structure",
                "limit": 0,
                "unique": true
            })
        );
    }

    #[test]
    fn explore_params_reject_unique_outside_structure() {
        let params = ExploreParams {
            node_key: "heading:alpha.org:3".to_owned(),
            lens: ExplorationLens::Refs,
            limit: 25,
            unique: true,
        };

        assert_eq!(
            params.validation_error().as_deref(),
            Some("explore unique is only supported for the structure lens")
        );
    }

    #[test]
    fn explore_result_serializes_declared_sections() {
        let result = ExploreResult {
            lens: ExplorationLens::Structure,
            sections: vec![ExplorationSection {
                kind: ExplorationSectionKind::Backlinks,
                entries: vec![ExplorationEntry::Backlink {
                    record: Box::new(BacklinkRecord {
                        source_note: NodeRecord {
                            node_key: "heading:source.org:5".to_owned(),
                            explicit_id: Some("source-id".to_owned()),
                            file_path: "source.org".to_owned(),
                            title: "Source".to_owned(),
                            outline_path: "Source".to_owned(),
                            aliases: Vec::new(),
                            tags: Vec::new(),
                            refs: Vec::new(),
                            todo_keyword: None,
                            scheduled_for: None,
                            deadline_for: None,
                            closed_at: None,
                            level: 1,
                            line: 5,
                            kind: NodeKind::Heading,
                            file_mtime_ns: 0,
                            backlink_count: 0,
                            forward_link_count: 0,
                        },
                        source_anchor: None,
                        row: 5,
                        col: 2,
                        preview: "[[id:target]]".to_owned(),
                        explanation: ExplorationExplanation::Backlink,
                    }),
                }],
            }],
        };

        assert_eq!(
            serde_json::to_value(result).expect("explore result should serialize"),
            json!({
                "lens": "structure",
                "sections": [{
                    "kind": "backlinks",
                    "entries": [{
                        "kind": "backlink",
                        "source_note": {
                            "node_key": "heading:source.org:5",
                            "explicit_id": "source-id",
                            "file_path": "source.org",
                            "title": "Source",
                            "outline_path": "Source",
                            "aliases": [],
                            "tags": [],
                            "refs": [],
                            "todo_keyword": null,
                            "scheduled_for": null,
                            "deadline_for": null,
                            "closed_at": null,
                            "level": 1,
                            "line": 5,
                            "kind": "heading",
                            "file_mtime_ns": 0,
                            "backlink_count": 0,
                            "forward_link_count": 0
                        },
                        "source_anchor": null,
                        "row": 5,
                        "col": 2,
                        "preview": "[[id:target]]",
                        "explanation": { "kind": "backlink" }
                    }]
                }]
            })
        );
    }

    #[test]
    fn compare_notes_params_round_trip() {
        let params: CompareNotesParams = serde_json::from_value(json!({
            "left_node_key": "heading:left.org:3",
            "right_node_key": "heading:right.org:7",
            "limit": 0
        }))
        .expect("compare-notes params should deserialize");

        assert_eq!(params.left_node_key, "heading:left.org:3");
        assert_eq!(params.right_node_key, "heading:right.org:7");
        assert_eq!(params.normalized_limit(), 1);

        assert_eq!(
            serde_json::to_value(&params).expect("compare-notes params should serialize"),
            json!({
                "left_node_key": "heading:left.org:3",
                "right_node_key": "heading:right.org:7",
                "limit": 0
            })
        );
    }

    #[test]
    fn note_comparison_explanation_serializes_connectors() {
        assert_eq!(
            serde_json::to_value(NoteComparisonExplanation::IndirectConnector {
                direction: ComparisonConnectorDirection::Bidirectional,
            })
            .expect("connector explanation should serialize"),
            json!({
                "kind": "indirect-connector",
                "direction": "bidirectional"
            })
        );

        assert_eq!(
            serde_json::to_value(NoteComparisonExplanation::PlanningTension)
                .expect("planning-tension explanation should serialize"),
            json!({
                "kind": "planning-tension"
            })
        );
    }

    #[test]
    fn note_comparison_result_serializes_declared_sections() {
        let result = NoteComparisonResult {
            left_note: NodeRecord {
                node_key: "heading:left.org:3".to_owned(),
                explicit_id: Some("left-id".to_owned()),
                file_path: "left.org".to_owned(),
                title: "Left".to_owned(),
                outline_path: "Left".to_owned(),
                aliases: Vec::new(),
                tags: Vec::new(),
                refs: vec!["@shared2024".to_owned()],
                todo_keyword: None,
                scheduled_for: None,
                deadline_for: None,
                closed_at: None,
                level: 1,
                line: 3,
                kind: NodeKind::Heading,
                file_mtime_ns: 0,
                backlink_count: 0,
                forward_link_count: 0,
            },
            right_note: NodeRecord {
                node_key: "heading:right.org:7".to_owned(),
                explicit_id: Some("right-id".to_owned()),
                file_path: "right.org".to_owned(),
                title: "Right".to_owned(),
                outline_path: "Right".to_owned(),
                aliases: Vec::new(),
                tags: Vec::new(),
                refs: vec!["@shared2024".to_owned()],
                todo_keyword: None,
                scheduled_for: None,
                deadline_for: None,
                closed_at: None,
                level: 1,
                line: 7,
                kind: NodeKind::Heading,
                file_mtime_ns: 0,
                backlink_count: 0,
                forward_link_count: 0,
            },
            sections: vec![
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::SharedRefs,
                    entries: vec![NoteComparisonEntry::Reference {
                        record: Box::new(ComparisonReferenceRecord {
                            reference: "@shared2024".to_owned(),
                            explanation: NoteComparisonExplanation::SharedReference,
                        }),
                    }],
                },
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::PlanningTensions,
                    entries: vec![
                        NoteComparisonEntry::PlanningRelation {
                            record: Box::new(ComparisonPlanningRecord {
                                date: "2026-05-01".to_owned(),
                                left_field: PlanningField::Scheduled,
                                right_field: PlanningField::Deadline,
                                explanation: NoteComparisonExplanation::PlanningTension,
                            }),
                        },
                        NoteComparisonEntry::TaskState {
                            record: Box::new(ComparisonTaskStateRecord {
                                left_todo_keyword: "TODO".to_owned(),
                                right_todo_keyword: "NEXT".to_owned(),
                                explanation: NoteComparisonExplanation::ContrastingTaskState,
                            }),
                        },
                    ],
                },
            ],
        };

        assert_eq!(
            serde_json::to_value(result).expect("comparison result should serialize"),
            json!({
                "left_note": {
                    "node_key": "heading:left.org:3",
                    "explicit_id": "left-id",
                    "file_path": "left.org",
                    "title": "Left",
                    "outline_path": "Left",
                    "aliases": [],
                    "tags": [],
                    "refs": ["@shared2024"],
                    "todo_keyword": null,
                    "scheduled_for": null,
                    "deadline_for": null,
                    "closed_at": null,
                    "level": 1,
                    "line": 3,
                    "kind": "heading",
                    "file_mtime_ns": 0,
                    "backlink_count": 0,
                    "forward_link_count": 0
                },
                "right_note": {
                    "node_key": "heading:right.org:7",
                    "explicit_id": "right-id",
                    "file_path": "right.org",
                    "title": "Right",
                    "outline_path": "Right",
                    "aliases": [],
                    "tags": [],
                    "refs": ["@shared2024"],
                    "todo_keyword": null,
                    "scheduled_for": null,
                    "deadline_for": null,
                    "closed_at": null,
                    "level": 1,
                    "line": 7,
                    "kind": "heading",
                    "file_mtime_ns": 0,
                    "backlink_count": 0,
                    "forward_link_count": 0
                },
                "sections": [
                    {
                        "kind": "shared-refs",
                        "entries": [{
                            "kind": "reference",
                            "reference": "@shared2024",
                            "explanation": { "kind": "shared-reference" }
                        }]
                    },
                    {
                        "kind": "planning-tensions",
                        "entries": [
                            {
                                "kind": "planning-relation",
                                "date": "2026-05-01",
                                "left_field": "scheduled",
                                "right_field": "deadline",
                                "explanation": { "kind": "planning-tension" }
                            },
                            {
                                "kind": "task-state",
                                "left_todo_keyword": "TODO",
                                "right_todo_keyword": "NEXT",
                                "explanation": { "kind": "contrasting-task-state" }
                            }
                        ]
                    }
                ]
            })
        );
    }

    #[test]
    fn note_comparison_group_filters_declared_sections() {
        let result = NoteComparisonResult {
            left_note: NodeRecord {
                node_key: "heading:left.org:3".to_owned(),
                explicit_id: Some("left-id".to_owned()),
                file_path: "left.org".to_owned(),
                title: "Left".to_owned(),
                outline_path: "Left".to_owned(),
                aliases: Vec::new(),
                tags: Vec::new(),
                refs: Vec::new(),
                todo_keyword: None,
                scheduled_for: None,
                deadline_for: None,
                closed_at: None,
                level: 1,
                line: 3,
                kind: NodeKind::Heading,
                file_mtime_ns: 0,
                backlink_count: 0,
                forward_link_count: 0,
            },
            right_note: NodeRecord {
                node_key: "heading:right.org:7".to_owned(),
                explicit_id: Some("right-id".to_owned()),
                file_path: "right.org".to_owned(),
                title: "Right".to_owned(),
                outline_path: "Right".to_owned(),
                aliases: Vec::new(),
                tags: Vec::new(),
                refs: Vec::new(),
                todo_keyword: None,
                scheduled_for: None,
                deadline_for: None,
                closed_at: None,
                level: 1,
                line: 7,
                kind: NodeKind::Heading,
                file_mtime_ns: 0,
                backlink_count: 0,
                forward_link_count: 0,
            },
            sections: vec![
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::SharedRefs,
                    entries: Vec::new(),
                },
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::LeftOnlyRefs,
                    entries: Vec::new(),
                },
                NoteComparisonSection {
                    kind: NoteComparisonSectionKind::ContrastingTaskStates,
                    entries: Vec::new(),
                },
            ],
        };

        assert_eq!(
            result
                .filtered_to_group(NoteComparisonGroup::Overlap)
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![NoteComparisonSectionKind::SharedRefs]
        );
        assert_eq!(
            result
                .filtered_to_group(NoteComparisonGroup::Divergence)
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![NoteComparisonSectionKind::LeftOnlyRefs]
        );
        assert_eq!(
            result
                .filtered_to_group(NoteComparisonGroup::Tension)
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![NoteComparisonSectionKind::ContrastingTaskStates]
        );
    }

    #[test]
    fn saved_lens_view_artifact_round_trips_and_reuses_explore_validation() {
        let artifact = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "lens-focus".to_owned(),
                title: "Focus refs".to_owned(),
                summary: Some("Pinned refs view".to_owned()),
            },
            payload: ExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "file:focus.org".to_owned(),
                    current_node_key: "heading:focus.org:3".to_owned(),
                    lens: ExplorationLens::Refs,
                    limit: 0,
                    unique: false,
                    frozen_context: true,
                }),
            },
        };

        assert_eq!(artifact.kind(), ExplorationArtifactKind::LensView);
        assert_eq!(artifact.validation_error(), None);

        let serialized =
            serde_json::to_value(&artifact).expect("saved lens-view artifact should serialize");
        assert_eq!(
            serialized,
            json!({
                "artifact_id": "lens-focus",
                "title": "Focus refs",
                "summary": "Pinned refs view",
                "kind": "lens-view",
                "root_node_key": "file:focus.org",
                "current_node_key": "heading:focus.org:3",
                "lens": "refs",
                "limit": 0,
                "unique": false,
                "frozen_context": true
            })
        );

        let round_trip: SavedExplorationArtifact = serde_json::from_value(serialized)
            .expect("saved lens-view artifact should deserialize");
        assert_eq!(round_trip, artifact);

        let invalid = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "invalid-lens".to_owned(),
                title: "Invalid".to_owned(),
                summary: None,
            },
            payload: ExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "heading:focus.org:3".to_owned(),
                    current_node_key: "heading:focus.org:3".to_owned(),
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: true,
                    frozen_context: false,
                }),
            },
        };

        assert_eq!(
            invalid.validation_error().as_deref(),
            Some("explore unique is only supported for the structure lens")
        );

        let non_frozen_root_mismatch = SavedLensViewArtifact {
            root_node_key: "file:other.org".to_owned(),
            current_node_key: "heading:focus.org:3".to_owned(),
            lens: ExplorationLens::Refs,
            limit: 25,
            unique: false,
            frozen_context: false,
        };

        assert_eq!(
            non_frozen_root_mismatch.validation_error().as_deref(),
            Some("non-frozen lens-view artifacts must use current_node_key as root_node_key")
        );
    }

    #[test]
    fn saved_comparison_artifact_round_trips_with_group_semantics() {
        let artifact = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "compare-focus-neighbor".to_owned(),
                title: "Focus vs Neighbor".to_owned(),
                summary: None,
            },
            payload: ExplorationArtifactPayload::Comparison {
                artifact: Box::new(SavedComparisonArtifact {
                    root_node_key: "file:focus.org".to_owned(),
                    left_node_key: "heading:focus.org:3".to_owned(),
                    right_node_key: "heading:neighbor.org:7".to_owned(),
                    active_lens: ExplorationLens::Tasks,
                    structure_unique: false,
                    comparison_group: NoteComparisonGroup::Tension,
                    limit: 0,
                    frozen_context: true,
                }),
            },
        };

        assert_eq!(artifact.kind(), ExplorationArtifactKind::Comparison);
        assert_eq!(artifact.validation_error(), None);
        let serialized =
            serde_json::to_value(&artifact).expect("saved comparison artifact should serialize");
        assert_eq!(
            serialized,
            json!({
                "artifact_id": "compare-focus-neighbor",
                "title": "Focus vs Neighbor",
                "summary": null,
                "kind": "comparison",
                "root_node_key": "file:focus.org",
                "left_node_key": "heading:focus.org:3",
                "right_node_key": "heading:neighbor.org:7",
                "active_lens": "tasks",
                "structure_unique": false,
                "comparison_group": "tension",
                "limit": 0,
                "frozen_context": true
            })
        );
        let round_trip: SavedExplorationArtifact = serde_json::from_value(serialized)
            .expect("saved comparison artifact should deserialize");
        assert_eq!(round_trip, artifact);

        let invalid = SavedComparisonArtifact {
            root_node_key: "heading:focus.org:3".to_owned(),
            left_node_key: "heading:focus.org:3".to_owned(),
            right_node_key: "heading:focus.org:3".to_owned(),
            active_lens: ExplorationLens::Structure,
            structure_unique: false,
            comparison_group: NoteComparisonGroup::All,
            limit: 25,
            frozen_context: false,
        };

        assert_eq!(
            invalid.validation_error().as_deref(),
            Some("left_node_key and right_node_key must differ")
        );

        let non_frozen_root_mismatch = SavedComparisonArtifact {
            root_node_key: "heading:previous.org:1".to_owned(),
            left_node_key: "heading:focus.org:3".to_owned(),
            right_node_key: "heading:neighbor.org:7".to_owned(),
            active_lens: ExplorationLens::Structure,
            structure_unique: false,
            comparison_group: NoteComparisonGroup::All,
            limit: 25,
            frozen_context: false,
        };

        assert_eq!(
            non_frozen_root_mismatch.validation_error().as_deref(),
            Some("non-frozen comparison artifacts must use left_node_key as root_node_key")
        );
    }

    #[test]
    fn saved_trail_artifact_round_trips_and_preserves_detached_step() {
        let artifact = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "trail-focus".to_owned(),
                title: "Focus trail".to_owned(),
                summary: Some("Detached comparison branch".to_owned()),
            },
            payload: ExplorationArtifactPayload::Trail {
                artifact: Box::new(SavedTrailArtifact {
                    steps: vec![
                        SavedTrailStep::LensView {
                            artifact: Box::new(SavedLensViewArtifact {
                                root_node_key: "file:focus.org".to_owned(),
                                current_node_key: "heading:focus.org:3".to_owned(),
                                lens: ExplorationLens::Unresolved,
                                limit: 200,
                                unique: false,
                                frozen_context: true,
                            }),
                        },
                        SavedTrailStep::Comparison {
                            artifact: Box::new(SavedComparisonArtifact {
                                root_node_key: "file:focus.org".to_owned(),
                                left_node_key: "heading:focus.org:3".to_owned(),
                                right_node_key: "heading:neighbor.org:7".to_owned(),
                                active_lens: ExplorationLens::Refs,
                                structure_unique: false,
                                comparison_group: NoteComparisonGroup::Overlap,
                                limit: 100,
                                frozen_context: true,
                            }),
                        },
                    ],
                    cursor: 0,
                    detached_step: Some(Box::new(SavedTrailStep::Comparison {
                        artifact: Box::new(SavedComparisonArtifact {
                            root_node_key: "file:focus.org".to_owned(),
                            left_node_key: "heading:focus.org:3".to_owned(),
                            right_node_key: "heading:tension.org:9".to_owned(),
                            active_lens: ExplorationLens::Structure,
                            structure_unique: true,
                            comparison_group: NoteComparisonGroup::Tension,
                            limit: 100,
                            frozen_context: true,
                        }),
                    })),
                }),
            },
        };

        assert_eq!(artifact.kind(), ExplorationArtifactKind::Trail);
        assert_eq!(artifact.validation_error(), None);

        let serialized =
            serde_json::to_value(&artifact).expect("saved trail artifact should serialize");
        let round_trip: SavedExplorationArtifact = serde_json::from_value(serialized.clone())
            .expect("saved trail artifact should deserialize");
        assert_eq!(round_trip, artifact);
        assert_eq!(serialized["kind"], json!("trail"));
        assert_eq!(serialized["steps"][0]["kind"], json!("lens-view"));
        assert_eq!(serialized["steps"][1]["kind"], json!("comparison"));
        assert_eq!(serialized["detached_step"]["kind"], json!("comparison"));
    }

    #[test]
    fn executed_exploration_artifacts_round_trip_with_trail_replay() {
        let executed = ExecutedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "executed-trail".to_owned(),
                title: "Executed trail".to_owned(),
                summary: Some("Replay result".to_owned()),
            },
            payload: ExecutedExplorationArtifactPayload::Trail {
                artifact: Box::new(SavedTrailArtifact {
                    steps: vec![SavedTrailStep::LensView {
                        artifact: Box::new(SavedLensViewArtifact {
                            root_node_key: "file:focus.org".to_owned(),
                            current_node_key: "file:focus.org".to_owned(),
                            lens: ExplorationLens::Structure,
                            limit: 5,
                            unique: false,
                            frozen_context: false,
                        }),
                    }],
                    cursor: 0,
                    detached_step: None,
                }),
                replay: Box::new(TrailReplayResult {
                    steps: vec![TrailReplayStepResult::LensView {
                        artifact: Box::new(SavedLensViewArtifact {
                            root_node_key: "file:focus.org".to_owned(),
                            current_node_key: "file:focus.org".to_owned(),
                            lens: ExplorationLens::Structure,
                            limit: 5,
                            unique: false,
                            frozen_context: false,
                        }),
                        root_note: Box::new(NodeRecord {
                            node_key: "file:focus.org".to_owned(),
                            explicit_id: Some("focus-id".to_owned()),
                            file_path: "focus.org".to_owned(),
                            title: "Focus".to_owned(),
                            outline_path: String::new(),
                            aliases: Vec::new(),
                            tags: Vec::new(),
                            refs: Vec::new(),
                            todo_keyword: None,
                            scheduled_for: None,
                            deadline_for: None,
                            closed_at: None,
                            level: 0,
                            line: 1,
                            kind: NodeKind::File,
                            file_mtime_ns: 123,
                            backlink_count: 1,
                            forward_link_count: 0,
                        }),
                        current_note: Box::new(NodeRecord {
                            node_key: "file:focus.org".to_owned(),
                            explicit_id: Some("focus-id".to_owned()),
                            file_path: "focus.org".to_owned(),
                            title: "Focus".to_owned(),
                            outline_path: String::new(),
                            aliases: Vec::new(),
                            tags: Vec::new(),
                            refs: Vec::new(),
                            todo_keyword: None,
                            scheduled_for: None,
                            deadline_for: None,
                            closed_at: None,
                            level: 0,
                            line: 1,
                            kind: NodeKind::File,
                            file_mtime_ns: 123,
                            backlink_count: 1,
                            forward_link_count: 0,
                        }),
                        result: Box::new(ExploreResult {
                            lens: ExplorationLens::Structure,
                            sections: vec![ExplorationSection {
                                kind: ExplorationSectionKind::Backlinks,
                                entries: vec![ExplorationEntry::Backlink {
                                    record: Box::new(BacklinkRecord {
                                        source_note: NodeRecord {
                                            node_key: "file:focus.org".to_owned(),
                                            explicit_id: Some("focus-id".to_owned()),
                                            file_path: "focus.org".to_owned(),
                                            title: "Focus".to_owned(),
                                            outline_path: String::new(),
                                            aliases: Vec::new(),
                                            tags: Vec::new(),
                                            refs: Vec::new(),
                                            todo_keyword: None,
                                            scheduled_for: None,
                                            deadline_for: None,
                                            closed_at: None,
                                            level: 0,
                                            line: 1,
                                            kind: NodeKind::File,
                                            file_mtime_ns: 123,
                                            backlink_count: 1,
                                            forward_link_count: 0,
                                        },
                                        source_anchor: None,
                                        row: 3,
                                        col: 9,
                                        preview: "Links to focus".to_owned(),
                                        explanation: ExplorationExplanation::Backlink,
                                    }),
                                }],
                            }],
                        }),
                    }],
                    cursor: 0,
                    detached_step: None,
                }),
            },
        };

        assert_eq!(executed.kind(), ExplorationArtifactKind::Trail);
        let serialized = serde_json::to_value(&executed)
            .expect("executed exploration artifact should serialize");
        assert_eq!(serialized["kind"], json!("trail"));
        assert_eq!(serialized["replay"]["steps"][0]["kind"], json!("lens-view"));

        let round_trip: ExecutedExplorationArtifact = serde_json::from_value(serialized)
            .expect("executed exploration artifact should deserialize");
        assert_eq!(round_trip, executed);
    }

    #[test]
    fn exploration_artifact_rpc_contracts_round_trip() {
        let artifact = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "lens/focus".to_owned(),
                title: "Lens Focus".to_owned(),
                summary: Some("Saved structure lens".to_owned()),
            },
            payload: ExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "file:focus.org".to_owned(),
                    current_node_key: "file:focus.org".to_owned(),
                    lens: ExplorationLens::Structure,
                    limit: 20,
                    unique: false,
                    frozen_context: false,
                }),
            },
        };
        let summary = ExplorationArtifactSummary::from(&artifact);
        let save_params = SaveExplorationArtifactParams {
            artifact: artifact.clone(),
        };
        let save_result = SaveExplorationArtifactResult {
            artifact: summary.clone(),
        };
        let list_result = ListExplorationArtifactsResult {
            artifacts: vec![summary.clone()],
        };
        let inspect_result = ExplorationArtifactResult {
            artifact: artifact.clone(),
        };
        let execute_result = ExecuteExplorationArtifactResult {
            artifact: ExecutedExplorationArtifact {
                metadata: artifact.metadata.clone(),
                payload: ExecutedExplorationArtifactPayload::LensView {
                    artifact: Box::new(SavedLensViewArtifact {
                        root_node_key: "file:focus.org".to_owned(),
                        current_node_key: "file:focus.org".to_owned(),
                        lens: ExplorationLens::Structure,
                        limit: 20,
                        unique: false,
                        frozen_context: false,
                    }),
                    root_note: Box::new(NodeRecord {
                        node_key: "file:focus.org".to_owned(),
                        explicit_id: None,
                        file_path: "focus.org".to_owned(),
                        title: "Focus".to_owned(),
                        outline_path: String::new(),
                        aliases: Vec::new(),
                        tags: Vec::new(),
                        refs: Vec::new(),
                        todo_keyword: None,
                        scheduled_for: None,
                        deadline_for: None,
                        closed_at: None,
                        level: 0,
                        line: 1,
                        kind: NodeKind::File,
                        file_mtime_ns: 0,
                        backlink_count: 0,
                        forward_link_count: 0,
                    }),
                    current_note: Box::new(NodeRecord {
                        node_key: "file:focus.org".to_owned(),
                        explicit_id: None,
                        file_path: "focus.org".to_owned(),
                        title: "Focus".to_owned(),
                        outline_path: String::new(),
                        aliases: Vec::new(),
                        tags: Vec::new(),
                        refs: Vec::new(),
                        todo_keyword: None,
                        scheduled_for: None,
                        deadline_for: None,
                        closed_at: None,
                        level: 0,
                        line: 1,
                        kind: NodeKind::File,
                        file_mtime_ns: 0,
                        backlink_count: 0,
                        forward_link_count: 0,
                    }),
                    result: Box::new(ExploreResult {
                        lens: ExplorationLens::Structure,
                        sections: Vec::new(),
                    }),
                },
            },
        };
        let delete_result = DeleteExplorationArtifactResult {
            artifact_id: "lens/focus".to_owned(),
        };
        let id_params = ExplorationArtifactIdParams {
            artifact_id: "lens/focus".to_owned(),
        };

        let save_json = serde_json::to_value(&save_params).expect("save params should serialize");
        assert_eq!(save_json["artifact"]["artifact_id"], json!("lens/focus"));
        assert_eq!(save_json["artifact"]["kind"], json!("lens-view"));

        let save_round_trip: SaveExplorationArtifactParams =
            serde_json::from_value(save_json).expect("save params should deserialize");
        assert_eq!(save_round_trip, save_params);

        let save_result_round_trip: SaveExplorationArtifactResult = serde_json::from_value(
            serde_json::to_value(&save_result).expect("save result should serialize"),
        )
        .expect("save result should deserialize");
        assert_eq!(save_result_round_trip, save_result);

        let summary_json = serde_json::to_value(&summary).expect("summary should serialize");
        assert_eq!(summary_json["kind"], json!("lens-view"));

        let list_round_trip: ListExplorationArtifactsResult = serde_json::from_value(
            serde_json::to_value(&list_result).expect("list result should serialize"),
        )
        .expect("list result should deserialize");
        assert_eq!(list_round_trip, list_result);

        let inspect_round_trip: ExplorationArtifactResult = serde_json::from_value(
            serde_json::to_value(&inspect_result).expect("inspect result should serialize"),
        )
        .expect("inspect result should deserialize");
        assert_eq!(inspect_round_trip, inspect_result);

        let execute_round_trip: ExecuteExplorationArtifactResult = serde_json::from_value(
            serde_json::to_value(&execute_result).expect("execute result should serialize"),
        )
        .expect("execute result should deserialize");
        assert_eq!(execute_round_trip, execute_result);

        let delete_round_trip: DeleteExplorationArtifactResult = serde_json::from_value(
            serde_json::to_value(&delete_result).expect("delete result should serialize"),
        )
        .expect("delete result should deserialize");
        assert_eq!(delete_round_trip, delete_result);

        let id_round_trip: ExplorationArtifactIdParams = serde_json::from_value(
            serde_json::to_value(&id_params).expect("id params should serialize"),
        )
        .expect("id params should deserialize");
        assert_eq!(id_round_trip, id_params);
    }

    #[test]
    fn saved_exploration_artifacts_reject_malformed_metadata_and_trails() {
        let blank_metadata = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: " ".to_owned(),
                title: "Title".to_owned(),
                summary: None,
            },
            payload: ExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "file:focus.org".to_owned(),
                    current_node_key: "heading:focus.org:3".to_owned(),
                    lens: ExplorationLens::Structure,
                    limit: 10,
                    unique: false,
                    frozen_context: false,
                }),
            },
        };

        assert_eq!(
            blank_metadata.validation_error().as_deref(),
            Some("artifact_id must not be empty")
        );

        let padded_metadata = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: " focus ".to_owned(),
                title: "Title".to_owned(),
                summary: None,
            },
            payload: ExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "file:focus.org".to_owned(),
                    current_node_key: "heading:focus.org:3".to_owned(),
                    lens: ExplorationLens::Structure,
                    limit: 10,
                    unique: false,
                    frozen_context: false,
                }),
            },
        };
        assert_eq!(
            padded_metadata.validation_error().as_deref(),
            Some("artifact_id must not have leading or trailing whitespace")
        );

        let empty_trail = SavedTrailArtifact {
            steps: Vec::new(),
            cursor: 0,
            detached_step: None,
        };
        assert_eq!(
            empty_trail.validation_error().as_deref(),
            Some("trail artifacts must contain at least one step")
        );

        let out_of_bounds_cursor = SavedTrailArtifact {
            steps: vec![SavedTrailStep::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "file:focus.org".to_owned(),
                    current_node_key: "heading:focus.org:3".to_owned(),
                    lens: ExplorationLens::Structure,
                    limit: 10,
                    unique: false,
                    frozen_context: false,
                }),
            }],
            cursor: 1,
            detached_step: None,
        };
        assert_eq!(
            out_of_bounds_cursor.validation_error().as_deref(),
            Some("trail cursor must point to an existing step")
        );

        let invalid_nested_step = SavedTrailArtifact {
            steps: vec![SavedTrailStep::Comparison {
                artifact: Box::new(SavedComparisonArtifact {
                    root_node_key: "heading:focus.org:3".to_owned(),
                    left_node_key: "heading:focus.org:3".to_owned(),
                    right_node_key: "heading:focus.org:3".to_owned(),
                    active_lens: ExplorationLens::Structure,
                    structure_unique: false,
                    comparison_group: NoteComparisonGroup::All,
                    limit: 10,
                    frozen_context: false,
                }),
            }],
            cursor: 0,
            detached_step: None,
        };
        assert_eq!(
            invalid_nested_step.validation_error().as_deref(),
            Some("trail step 0 is invalid: left_node_key and right_node_key must differ")
        );

        let attached_detached_step = SavedTrailArtifact {
            steps: vec![SavedTrailStep::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "heading:focus.org:3".to_owned(),
                    current_node_key: "heading:focus.org:3".to_owned(),
                    lens: ExplorationLens::Structure,
                    limit: 10,
                    unique: false,
                    frozen_context: false,
                }),
            }],
            cursor: 0,
            detached_step: Some(Box::new(SavedTrailStep::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "heading:focus.org:3".to_owned(),
                    current_node_key: "heading:focus.org:3".to_owned(),
                    lens: ExplorationLens::Structure,
                    limit: 10,
                    unique: false,
                    frozen_context: false,
                }),
            })),
        };

        assert_eq!(
            attached_detached_step.validation_error().as_deref(),
            Some("detached trail step must not duplicate any recorded trail step")
        );
    }

    #[test]
    fn exploration_artifact_id_params_reject_padded_ids() {
        let padded = ExplorationArtifactIdParams {
            artifact_id: " lens/focus ".to_owned(),
        };
        assert_eq!(
            padded.validation_error().as_deref(),
            Some("artifact_id must not have leading or trailing whitespace")
        );
    }

    #[test]
    fn search_nodes_params_support_kebab_case_sort_names() {
        let params: SearchNodesParams = serde_json::from_value(json!({
            "query": "alpha",
            "limit": 10,
            "sort": "forward-link-count"
        }))
        .expect("search node params should deserialize");

        assert_eq!(params.query, "alpha");
        assert_eq!(params.limit, 10);
        assert_eq!(params.sort, Some(SearchNodesSort::ForwardLinkCount));

        assert_eq!(
            serde_json::to_value(&params).expect("search node params should serialize"),
            json!({
                "query": "alpha",
                "limit": 10,
                "sort": "forward-link-count"
            })
        );
    }

    #[test]
    fn search_nodes_params_default_to_unspecified_sort() {
        let params: SearchNodesParams =
            serde_json::from_value(json!({ "query": "alpha", "limit": 10 }))
                .expect("search node params should deserialize");

        assert_eq!(params.sort, None);
    }

    #[test]
    fn node_from_title_or_alias_params_round_trip_without_scope() {
        let params: NodeFromTitleOrAliasParams =
            serde_json::from_value(json!({ "title_or_alias": "alpha", "nocase": true }))
                .expect("title-or-alias params should deserialize");

        assert_eq!(params.title_or_alias, "alpha");
        assert!(params.nocase);

        assert_eq!(
            serde_json::to_value(&params).expect("title-or-alias params should serialize"),
            json!({
                "title_or_alias": "alpha",
                "nocase": true
            })
        );
    }

    #[test]
    fn node_from_key_params_round_trip() {
        let params: NodeFromKeyParams =
            serde_json::from_value(json!({ "node_key": "file:alpha.org" }))
                .expect("node-from-key params should deserialize");

        assert_eq!(params.node_key, "file:alpha.org");

        assert_eq!(
            serde_json::to_value(&params).expect("node-from-key params should serialize"),
            json!({
                "node_key": "file:alpha.org"
            })
        );
    }

    #[test]
    fn unlinked_references_params_normalize_limit() {
        let params: UnlinkedReferencesParams =
            serde_json::from_value(json!({ "node_key": "heading:alpha.org:3", "limit": 0 }))
                .expect("unlinked reference params should deserialize");

        assert_eq!(params.node_key, "heading:alpha.org:3");
        assert_eq!(params.normalized_limit(), 1);

        assert_eq!(
            serde_json::to_value(&params).expect("unlinked reference params should serialize"),
            json!({
                "node_key": "heading:alpha.org:3",
                "limit": 0
            })
        );
    }
}
