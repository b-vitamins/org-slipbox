use std::collections::BTreeMap;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    #[serde(default = "default_artifact_overwrite")]
    pub overwrite: bool,
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
pub struct WorkflowMetadata {
    pub workflow_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
}

impl WorkflowMetadata {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_id_field(&self.workflow_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSummary {
    #[serde(flatten)]
    pub metadata: WorkflowMetadata,
    pub step_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowInputKind {
    NoteTarget,
    FocusTarget,
}

impl WorkflowInputKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NoteTarget => "note-target",
            Self::FocusTarget => "focus-target",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowInputSpec {
    pub input_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub kind: WorkflowInputKind,
}

impl WorkflowInputSpec {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_input_id_field(&self.input_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowInputAssignment {
    pub input_id: String,
    #[serde(flatten)]
    pub target: WorkflowResolveTarget,
}

impl WorkflowInputAssignment {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_input_id_field(&self.input_id).or_else(|| match &self.target {
            WorkflowResolveTarget::Input { .. } => Some(
                "workflow input assignments cannot reference another workflow input".to_owned(),
            ),
            WorkflowResolveTarget::Id { id } => validate_required_text_field(id, "id"),
            WorkflowResolveTarget::Title { title } => validate_required_text_field(title, "title"),
            WorkflowResolveTarget::Reference { reference } => {
                validate_required_text_field(reference, "reference")
            }
            WorkflowResolveTarget::NodeKey { node_key } => {
                validate_required_text_field(node_key, "node_key")
            }
        })
    }
}

pub const BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID: &str = "workflow/builtin/context-sweep";
pub const BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID: &str = "workflow/builtin/unresolved-sweep";
pub const BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID: &str = "workflow/builtin/periodic-review";
pub const BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID: &str =
    "workflow/builtin/weak-integration-review";
pub const BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID: &str =
    "workflow/builtin/comparison-tension-review";
pub const WORKFLOW_SPEC_COMPATIBILITY_VERSION: u32 = 1;

#[must_use]
pub const fn default_workflow_spec_compatibility_version() -> u32 {
    WORKFLOW_SPEC_COMPATIBILITY_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSpecCompatibility {
    #[serde(default = "default_workflow_spec_compatibility_version")]
    pub version: u32,
}

impl Default for WorkflowSpecCompatibility {
    fn default() -> Self {
        Self {
            version: WORKFLOW_SPEC_COMPATIBILITY_VERSION,
        }
    }
}

impl WorkflowSpecCompatibility {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        if self.version == 0 {
            return Some(
                "workflow spec compatibility version must be greater than zero".to_owned(),
            );
        }
        (self.version > WORKFLOW_SPEC_COMPATIBILITY_VERSION).then(|| {
            format!(
                "unsupported workflow spec compatibility version {}; supported version is {}",
                self.version, WORKFLOW_SPEC_COMPATIBILITY_VERSION
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSpecCompatibilityEnvelope {
    #[serde(default)]
    pub workflow_id: Option<String>,
    #[serde(default)]
    pub compatibility: WorkflowSpecCompatibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowStepKind {
    Resolve,
    Explore,
    Compare,
    ArtifactRun,
    ArtifactSave,
}

impl WorkflowStepKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Resolve => "resolve",
            Self::Explore => "explore",
            Self::Compare => "compare",
            Self::ArtifactRun => "artifact-run",
            Self::ArtifactSave => "artifact-save",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowResolveTarget {
    Id { id: String },
    Title { title: String },
    Reference { reference: String },
    NodeKey { node_key: String },
    Input { input_id: String },
}

impl WorkflowResolveTarget {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::Id { id } => validate_required_text_field(id, "id"),
            Self::Title { title } => validate_required_text_field(title, "title"),
            Self::Reference { reference } => validate_required_text_field(reference, "reference"),
            Self::NodeKey { node_key } => validate_required_text_field(node_key, "node_key"),
            Self::Input { input_id } => validate_workflow_input_id_field(input_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepRef {
    pub step_id: String,
}

impl WorkflowStepRef {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_step_id_field(&self.step_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowExploreFocus {
    NodeKey { node_key: String },
    Input { input_id: String },
    ResolvedStep { step_id: String },
}

impl WorkflowExploreFocus {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::NodeKey { node_key } => validate_required_text_field(node_key, "node_key"),
            Self::Input { input_id } => validate_workflow_input_id_field(input_id),
            Self::ResolvedStep { step_id } => validate_workflow_step_id_field(step_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowArtifactSaveSource {
    ExploreStep { step_id: String },
    CompareStep { step_id: String },
}

impl WorkflowArtifactSaveSource {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::ExploreStep { step_id } | Self::CompareStep { step_id } => {
                validate_workflow_step_id_field(step_id)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowStepPayload {
    Resolve {
        target: WorkflowResolveTarget,
    },
    Explore {
        focus: WorkflowExploreFocus,
        lens: ExplorationLens,
        #[serde(default = "default_backlink_limit")]
        limit: usize,
        #[serde(default)]
        unique: bool,
    },
    Compare {
        left: WorkflowStepRef,
        right: WorkflowStepRef,
        #[serde(default)]
        group: NoteComparisonGroup,
        #[serde(default = "default_backlink_limit")]
        limit: usize,
    },
    ArtifactRun {
        artifact_id: String,
    },
    ArtifactSave {
        source: WorkflowArtifactSaveSource,
        #[serde(flatten)]
        metadata: ExplorationArtifactMetadata,
        #[serde(default = "default_artifact_overwrite")]
        overwrite: bool,
    },
}

impl WorkflowStepPayload {
    #[must_use]
    pub fn kind(&self) -> WorkflowStepKind {
        match self {
            Self::Resolve { .. } => WorkflowStepKind::Resolve,
            Self::Explore { .. } => WorkflowStepKind::Explore,
            Self::Compare { .. } => WorkflowStepKind::Compare,
            Self::ArtifactRun { .. } => WorkflowStepKind::ArtifactRun,
            Self::ArtifactSave { .. } => WorkflowStepKind::ArtifactSave,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::Resolve { target } => target.validation_error(),
            Self::Explore {
                focus,
                lens,
                limit,
                unique,
            } => focus.validation_error().or_else(|| {
                ExploreParams {
                    node_key: "__workflow_focus__".to_owned(),
                    lens: *lens,
                    limit: *limit,
                    unique: *unique,
                }
                .validation_error()
            }),
            Self::Compare {
                left,
                right,
                group: _,
                limit: _,
            } => left
                .validation_error()
                .or_else(|| right.validation_error())
                .or_else(|| {
                    (left.step_id == right.step_id).then(|| {
                        "compare left and right must reference distinct resolve steps".to_owned()
                    })
                }),
            Self::ArtifactRun { artifact_id } => validate_artifact_id_field(artifact_id),
            Self::ArtifactSave {
                source,
                metadata,
                overwrite: _,
            } => source
                .validation_error()
                .or_else(|| metadata.validation_error()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepSpec {
    pub step_id: String,
    #[serde(flatten)]
    pub payload: WorkflowStepPayload,
}

impl WorkflowStepSpec {
    #[must_use]
    pub fn kind(&self) -> WorkflowStepKind {
        self.payload.kind()
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_step_id_field(&self.step_id).or_else(|| self.payload.validation_error())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSpec {
    #[serde(flatten)]
    pub metadata: WorkflowMetadata,
    #[serde(default)]
    pub compatibility: WorkflowSpecCompatibility,
    #[serde(default)]
    pub inputs: Vec<WorkflowInputSpec>,
    pub steps: Vec<WorkflowStepSpec>,
}

impl WorkflowSpec {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.compatibility
            .validation_error()
            .or_else(|| self.metadata.validation_error())
            .or_else(|| {
                let mut seen_inputs: Vec<(&str, WorkflowInputKind)> =
                    Vec::with_capacity(self.inputs.len());
                for (index, input) in self.inputs.iter().enumerate() {
                    if let Some(error) = input.validation_error() {
                        return Some(format!("workflow input {index} is invalid: {error}"));
                    }
                    if seen_inputs
                        .iter()
                        .any(|(input_id, _)| *input_id == input.input_id)
                    {
                        return Some(format!(
                            "workflow input {index} reuses duplicate input_id {}",
                            input.input_id
                        ));
                    }
                    seen_inputs.push((input.input_id.as_str(), input.kind));
                }

                if self.steps.is_empty() {
                    return Some("workflows must contain at least one step".to_owned());
                }

                let mut seen: Vec<(&str, WorkflowStepKind)> = Vec::with_capacity(self.steps.len());
                for (index, step) in self.steps.iter().enumerate() {
                    if let Some(error) = step.validation_error() {
                        return Some(format!("workflow step {index} is invalid: {error}"));
                    }
                    if seen.iter().any(|(step_id, _)| *step_id == step.step_id) {
                        return Some(format!(
                            "workflow step {index} reuses duplicate step_id {}",
                            step.step_id
                        ));
                    }
                    if let Some(error) =
                        validate_workflow_step_references(&step.payload, &seen, &seen_inputs)
                    {
                        return Some(format!("workflow step {index} is invalid: {error}"));
                    }
                    seen.push((step.step_id.as_str(), step.kind()));
                }
                None
            })
    }

    #[must_use]
    pub fn has_unsupported_compatibility_version(&self) -> bool {
        self.compatibility.version > WORKFLOW_SPEC_COMPATIBILITY_VERSION
    }

    #[must_use]
    pub fn input_assignments_validation_error(
        &self,
        inputs: &[WorkflowInputAssignment],
    ) -> Option<String> {
        let mut seen_assignments: Vec<&str> = Vec::with_capacity(inputs.len());
        for (index, input) in inputs.iter().enumerate() {
            if let Some(error) = input.validation_error() {
                return Some(format!(
                    "workflow input assignment {index} is invalid: {error}"
                ));
            }
            if seen_assignments
                .iter()
                .any(|input_id| *input_id == input.input_id)
            {
                return Some(format!(
                    "workflow input assignment {index} reuses duplicate input_id {}",
                    input.input_id
                ));
            }
            if !self
                .inputs
                .iter()
                .any(|declared| declared.input_id == input.input_id)
            {
                return Some(format!(
                    "workflow input assignment {index} references unknown input_id {}",
                    input.input_id
                ));
            }
            seen_assignments.push(input.input_id.as_str());
        }

        self.inputs
            .iter()
            .find(|input| !seen_assignments.contains(&input.input_id.as_str()))
            .map(|input| format!("workflow input {} must be assigned", input.input_id))
    }
}

impl From<&WorkflowSpec> for WorkflowSummary {
    fn from(workflow: &WorkflowSpec) -> Self {
        Self {
            metadata: workflow.metadata.clone(),
            step_count: workflow.steps.len(),
        }
    }
}

#[must_use]
pub fn built_in_workflows() -> Vec<WorkflowSpec> {
    vec![
        built_in_context_sweep_workflow(),
        built_in_unresolved_sweep_workflow(),
        built_in_periodic_review_workflow(),
        built_in_weak_integration_review_workflow(),
        built_in_comparison_tension_workflow(),
    ]
}

#[must_use]
pub fn built_in_workflow(workflow_id: &str) -> Option<WorkflowSpec> {
    built_in_workflows()
        .into_iter()
        .find(|workflow| workflow.metadata.workflow_id == workflow_id)
}

#[must_use]
pub fn built_in_workflow_summaries() -> Vec<WorkflowSummary> {
    built_in_workflows()
        .into_iter()
        .map(|workflow| WorkflowSummary::from(&workflow))
        .collect()
}

fn built_in_context_sweep_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned(),
            title: "Context Sweep".to_owned(),
            summary: Some(
                "Resolve a focus note and gather structural, reference, and dormant context."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus target".to_owned(),
            summary: Some("Note or anchor target to inspect".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-structure".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Structure,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-refs".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-dormant".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Dormant,
                    limit: 25,
                    unique: false,
                },
            },
        ],
    }
}

fn built_in_unresolved_sweep_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID.to_owned(),
            title: "Unresolved Sweep".to_owned(),
            summary: Some(
                "Resolve a focus note and sweep unresolved, task, and time context around it."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus target".to_owned(),
            summary: Some("Note or anchor target to audit for unresolved pressure".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "explore-unresolved".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Unresolved,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-tasks".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Tasks,
                    limit: 25,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "explore-time".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Time,
                    limit: 25,
                    unique: false,
                },
            },
        ],
    }
}

fn built_in_periodic_review_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID.to_owned(),
            title: "Periodic Review".to_owned(),
            summary: Some(
                "Run a recurring review around a focus target across unresolved work, planning, references, and dormant context."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Review focus".to_owned(),
            summary: Some("Note or anchor target anchoring the recurring review".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "review-unresolved".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Unresolved,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-time".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Time,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-tasks".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Tasks,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-refs".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-dormant".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Dormant,
                    limit: 50,
                    unique: false,
                },
            },
        ],
    }
}

fn built_in_weak_integration_review_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID.to_owned(),
            title: "Weak Integration Review".to_owned(),
            summary: Some(
                "Review weakly integrated, dormant, and bridgeable material around a focus note."
                    .to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Integration focus".to_owned(),
            summary: Some(
                "Note or anchor target whose surrounding integration should be reviewed".to_owned(),
            ),
            kind: WorkflowInputKind::FocusTarget,
        }],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "review-weak-integration".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Unresolved,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-dormant".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Dormant,
                    limit: 50,
                    unique: false,
                },
            },
            WorkflowStepSpec {
                step_id: "review-bridges".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Bridges,
                    limit: 50,
                    unique: false,
                },
            },
        ],
    }
}

fn built_in_comparison_tension_workflow() -> WorkflowSpec {
    WorkflowSpec {
        metadata: WorkflowMetadata {
            workflow_id: BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID.to_owned(),
            title: "Comparison Tension Review".to_owned(),
            summary: Some(
                "Resolve two notes and review both tension and overlap together.".to_owned(),
            ),
        },
        compatibility: WorkflowSpecCompatibility::default(),
        inputs: vec![
            WorkflowInputSpec {
                input_id: "left".to_owned(),
                title: "Left note".to_owned(),
                summary: Some("First exact note target".to_owned()),
                kind: WorkflowInputKind::NoteTarget,
            },
            WorkflowInputSpec {
                input_id: "right".to_owned(),
                title: "Right note".to_owned(),
                summary: Some("Second exact note target".to_owned()),
                kind: WorkflowInputKind::NoteTarget,
            },
        ],
        steps: vec![
            WorkflowStepSpec {
                step_id: "resolve-left".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "left".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "resolve-right".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "right".to_owned(),
                    },
                },
            },
            WorkflowStepSpec {
                step_id: "compare-tension".to_owned(),
                payload: WorkflowStepPayload::Compare {
                    left: WorkflowStepRef {
                        step_id: "resolve-left".to_owned(),
                    },
                    right: WorkflowStepRef {
                        step_id: "resolve-right".to_owned(),
                    },
                    group: NoteComparisonGroup::Tension,
                    limit: 25,
                },
            },
            WorkflowStepSpec {
                step_id: "compare-overlap".to_owned(),
                payload: WorkflowStepPayload::Compare {
                    left: WorkflowStepRef {
                        step_id: "resolve-left".to_owned(),
                    },
                    right: WorkflowStepRef {
                        step_id: "resolve-right".to_owned(),
                    },
                    group: NoteComparisonGroup::Overlap,
                    limit: 25,
                },
            },
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowStepReportPayload {
    Resolve {
        node: Box<NodeRecord>,
    },
    Explore {
        focus_node_key: String,
        result: Box<ExploreResult>,
    },
    Compare {
        left_node: Box<NodeRecord>,
        right_node: Box<NodeRecord>,
        result: Box<NoteComparisonResult>,
    },
    ArtifactRun {
        artifact: Box<ExecutedExplorationArtifact>,
    },
    ArtifactSave {
        artifact: Box<ExplorationArtifactSummary>,
    },
}

impl WorkflowStepReportPayload {
    #[must_use]
    pub fn kind(&self) -> WorkflowStepKind {
        match self {
            Self::Resolve { .. } => WorkflowStepKind::Resolve,
            Self::Explore { .. } => WorkflowStepKind::Explore,
            Self::Compare { .. } => WorkflowStepKind::Compare,
            Self::ArtifactRun { .. } => WorkflowStepKind::ArtifactRun,
            Self::ArtifactSave { .. } => WorkflowStepKind::ArtifactSave,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepReport {
    pub step_id: String,
    #[serde(flatten)]
    pub payload: WorkflowStepReportPayload,
}

impl WorkflowStepReport {
    #[must_use]
    pub fn kind(&self) -> WorkflowStepKind {
        self.payload.kind()
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_step_id_field(&self.step_id).or_else(|| match &self.payload {
            WorkflowStepReportPayload::Resolve { node } => {
                validate_required_text_field(&node.node_key, "node_key")
            }
            WorkflowStepReportPayload::Explore {
                focus_node_key,
                result: _,
            } => validate_required_text_field(focus_node_key, "focus_node_key"),
            WorkflowStepReportPayload::Compare {
                left_node,
                right_node,
                result: _,
            } => validate_required_text_field(&left_node.node_key, "left_node_key")
                .or_else(|| validate_required_text_field(&right_node.node_key, "right_node_key")),
            WorkflowStepReportPayload::ArtifactRun { artifact } => {
                artifact.metadata.validation_error()
            }
            WorkflowStepReportPayload::ArtifactSave { artifact } => {
                artifact.metadata.validation_error()
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionResult {
    pub workflow: WorkflowSummary,
    pub steps: Vec<WorkflowStepReport>,
}

impl WorkflowExecutionResult {
    #[must_use]
    pub fn report_lines(&self) -> Vec<WorkflowReportLine> {
        let mut lines = Vec::with_capacity(self.steps.len() + 1);
        lines.push(WorkflowReportLine::Workflow {
            workflow: self.workflow.clone(),
        });
        lines.extend(
            self.steps
                .iter()
                .cloned()
                .map(|step| WorkflowReportLine::Step { step }),
        );
        lines
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkflowReportLine {
    Workflow { workflow: WorkflowSummary },
    Step { step: WorkflowStepReport },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowIdParams {
    pub workflow_id: String,
}

impl WorkflowIdParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_id_field(&self.workflow_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListWorkflowsParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCatalogIssue {
    pub path: String,
    pub kind: WorkflowCatalogIssueKind,
    #[serde(default)]
    pub workflow_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowCatalogIssueKind {
    Directory,
    Io,
    MalformedJson,
    UnsupportedVersion,
    InvalidSpec,
    DuplicateWorkflowId,
}

impl WorkflowCatalogIssueKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Io => "io",
            Self::MalformedJson => "malformed-json",
            Self::UnsupportedVersion => "unsupported-version",
            Self::InvalidSpec => "invalid-spec",
            Self::DuplicateWorkflowId => "duplicate-workflow-id",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListWorkflowsResult {
    pub workflows: Vec<WorkflowSummary>,
    #[serde(default)]
    pub issues: Vec<WorkflowCatalogIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub workflow: WorkflowSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunWorkflowParams {
    pub workflow_id: String,
    #[serde(default)]
    pub inputs: Vec<WorkflowInputAssignment>,
}

impl RunWorkflowParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_id_field(&self.workflow_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunWorkflowResult {
    pub result: WorkflowExecutionResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CorpusAuditKind {
    DanglingLinks,
    DuplicateTitles,
    OrphanNotes,
    WeaklyIntegratedNotes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusAuditParams {
    pub audit: CorpusAuditKind,
    #[serde(default = "default_audit_limit")]
    pub limit: usize,
}

impl CorpusAuditParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DanglingLinkAuditRecord {
    pub source: AnchorRecord,
    pub missing_explicit_id: String,
    pub line: u32,
    pub column: u32,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateTitleAuditRecord {
    pub title: String,
    pub notes: Vec<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteConnectivityAuditRecord {
    pub note: NodeRecord,
    pub reference_count: usize,
    pub backlink_count: usize,
    pub forward_link_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CorpusAuditEntry {
    DanglingLink {
        record: Box<DanglingLinkAuditRecord>,
    },
    DuplicateTitle {
        record: Box<DuplicateTitleAuditRecord>,
    },
    OrphanNote {
        record: Box<NoteConnectivityAuditRecord>,
    },
    WeaklyIntegratedNote {
        record: Box<NoteConnectivityAuditRecord>,
    },
}

impl CorpusAuditEntry {
    #[must_use]
    pub const fn kind(&self) -> CorpusAuditKind {
        match self {
            Self::DanglingLink { .. } => CorpusAuditKind::DanglingLinks,
            Self::DuplicateTitle { .. } => CorpusAuditKind::DuplicateTitles,
            Self::OrphanNote { .. } => CorpusAuditKind::OrphanNotes,
            Self::WeaklyIntegratedNote { .. } => CorpusAuditKind::WeaklyIntegratedNotes,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::DanglingLink { record } => {
                validate_required_text_field(&record.source.node_key, "source.node_key")
                    .or_else(|| {
                        validate_required_text_field(
                            &record.missing_explicit_id,
                            "missing_explicit_id",
                        )
                    })
                    .or_else(|| validate_required_text_field(&record.preview, "preview"))
            }
            Self::DuplicateTitle { record } => validate_required_text_field(&record.title, "title")
                .or_else(|| {
                    (record.notes.len() < 2).then(|| {
                        "duplicate-title findings must include at least two notes".to_owned()
                    })
                }),
            Self::OrphanNote { record } | Self::WeaklyIntegratedNote { record } => {
                validate_required_text_field(&record.note.node_key, "note.node_key")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusAuditResult {
    pub audit: CorpusAuditKind,
    pub entries: Vec<CorpusAuditEntry>,
}

impl CorpusAuditResult {
    #[must_use]
    pub fn report_lines(&self) -> Vec<CorpusAuditReportLine> {
        let mut lines = Vec::with_capacity(self.entries.len() + 1);
        lines.push(CorpusAuditReportLine::Audit { audit: self.audit });
        lines.extend(
            self.entries
                .iter()
                .cloned()
                .map(|entry| CorpusAuditReportLine::Entry { entry }),
        );
        lines
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CorpusAuditReportLine {
    Audit { audit: CorpusAuditKind },
    Entry { entry: CorpusAuditEntry },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewRunKind {
    Audit,
    Workflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRunMetadata {
    pub review_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
}

impl ReviewRunMetadata {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_id_field(&self.review_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReviewRunPayload {
    Audit {
        audit: CorpusAuditKind,
        #[serde(default)]
        limit: usize,
    },
    Workflow {
        workflow: WorkflowSummary,
        #[serde(default)]
        inputs: Vec<WorkflowInputAssignment>,
        step_ids: Vec<String>,
    },
}

impl ReviewRunPayload {
    #[must_use]
    pub const fn kind(&self) -> ReviewRunKind {
        match self {
            Self::Audit { .. } => ReviewRunKind::Audit,
            Self::Workflow { .. } => ReviewRunKind::Workflow,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::Audit { .. } => None,
            Self::Workflow {
                workflow,
                inputs,
                step_ids,
            } => workflow
                .metadata
                .validation_error()
                .or_else(|| validate_workflow_review_inputs(inputs))
                .or_else(|| validate_workflow_review_source(workflow, step_ids)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewFindingKind {
    Audit,
    WorkflowStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewFindingStatus {
    Open,
    Reviewed,
    Dismissed,
    Accepted,
}

impl ReviewFindingStatus {
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        self != next
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Reviewed => "reviewed",
            Self::Dismissed => "dismissed",
            Self::Accepted => "accepted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReviewFindingStatusCounts {
    pub open: usize,
    pub reviewed: usize,
    pub dismissed: usize,
    pub accepted: usize,
}

impl ReviewFindingStatusCounts {
    #[must_use]
    pub fn from_findings(findings: &[ReviewFinding]) -> Self {
        let mut counts = Self::default();
        for finding in findings {
            match finding.status {
                ReviewFindingStatus::Open => counts.open += 1,
                ReviewFindingStatus::Reviewed => counts.reviewed += 1,
                ReviewFindingStatus::Dismissed => counts.dismissed += 1,
                ReviewFindingStatus::Accepted => counts.accepted += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReviewFindingPayload {
    Audit { entry: Box<CorpusAuditEntry> },
    WorkflowStep { step: Box<WorkflowStepReport> },
}

impl ReviewFindingPayload {
    #[must_use]
    pub const fn kind(&self) -> ReviewFindingKind {
        match self {
            Self::Audit { .. } => ReviewFindingKind::Audit,
            Self::WorkflowStep { .. } => ReviewFindingKind::WorkflowStep,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::Audit { entry } => entry.validation_error(),
            Self::WorkflowStep { step } => step.validation_error(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub finding_id: String,
    pub status: ReviewFindingStatus,
    #[serde(flatten)]
    pub payload: ReviewFindingPayload,
}

impl ReviewFinding {
    #[must_use]
    pub fn kind(&self) -> ReviewFindingKind {
        self.payload.kind()
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_finding_id_field(&self.finding_id)
            .or_else(|| self.payload.validation_error())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingStatusTransition {
    pub review_id: String,
    pub finding_id: String,
    pub from_status: ReviewFindingStatus,
    pub to_status: ReviewFindingStatus,
}

impl ReviewFindingStatusTransition {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_id_field(&self.review_id)
            .or_else(|| validate_review_finding_id_field(&self.finding_id))
            .or_else(|| {
                (!self.from_status.can_transition_to(self.to_status))
                    .then(|| "review finding status transition must change status".to_owned())
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditRemediationConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AuditRemediationPreviewPayload {
    DanglingLink {
        source: Box<AnchorRecord>,
        missing_explicit_id: String,
        file_path: String,
        line: u32,
        column: u32,
        preview: String,
        suggestion: String,
        confidence: AuditRemediationConfidence,
        reason: String,
    },
    DuplicateTitle {
        title: String,
        notes: Vec<NodeRecord>,
        suggestion: String,
        confidence: AuditRemediationConfidence,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingRemediationPreview {
    pub review_id: String,
    pub finding_id: String,
    pub status: ReviewFindingStatus,
    #[serde(flatten)]
    pub payload: AuditRemediationPreviewPayload,
}

impl ReviewFindingRemediationPreview {
    pub fn from_review_finding(review_id: &str, finding: &ReviewFinding) -> Result<Self, String> {
        let payload = match &finding.payload {
            ReviewFindingPayload::Audit { entry } => match entry.as_ref() {
                CorpusAuditEntry::DanglingLink { record } => {
                    AuditRemediationPreviewPayload::DanglingLink {
                        source: record.source.clone().into(),
                        missing_explicit_id: record.missing_explicit_id.clone(),
                        file_path: record.source.file_path.clone(),
                        line: record.line,
                        column: record.column,
                        preview: record.preview.clone(),
                        suggestion: format!(
                            "Inspect the link to id:{} and either restore/create that target or update the link to an existing note ID.",
                            record.missing_explicit_id
                        ),
                        confidence: AuditRemediationConfidence::Medium,
                        reason: format!(
                            "Indexed link evidence points to missing explicit ID {} from {}:{}:{}.",
                            record.missing_explicit_id,
                            record.source.file_path,
                            record.line,
                            record.column
                        ),
                    }
                }
                CorpusAuditEntry::DuplicateTitle { record } => {
                    AuditRemediationPreviewPayload::DuplicateTitle {
                        title: record.title.clone(),
                        notes: record.notes.clone(),
                        suggestion:
                            "Disambiguate one or more duplicate titles so exact-title resolution is no longer ambiguous."
                                .to_owned(),
                        confidence: AuditRemediationConfidence::High,
                        reason: format!(
                            "Indexed title evidence found {} notes with the title {:?}.",
                            record.notes.len(),
                            record.title
                        ),
                    }
                }
                CorpusAuditEntry::OrphanNote { .. } => {
                    return Err(
                        "review finding has no remediation preview for orphan-note evidence"
                            .to_owned(),
                    );
                }
                CorpusAuditEntry::WeaklyIntegratedNote { .. } => {
                    return Err(
                        "review finding has no remediation preview for weakly-integrated-note evidence"
                            .to_owned(),
                    );
                }
            },
            ReviewFindingPayload::WorkflowStep { .. } => {
                return Err(
                    "review finding has no remediation preview for workflow-step evidence"
                        .to_owned(),
                );
            }
        };

        Ok(Self {
            review_id: review_id.to_owned(),
            finding_id: finding.finding_id.clone(),
            status: finding.status,
            payload,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRun {
    #[serde(flatten)]
    pub metadata: ReviewRunMetadata,
    #[serde(flatten)]
    pub payload: ReviewRunPayload,
    #[serde(default)]
    pub findings: Vec<ReviewFinding>,
}

impl ReviewRun {
    #[must_use]
    pub fn kind(&self) -> ReviewRunKind {
        self.payload.kind()
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.metadata
            .validation_error()
            .or_else(|| self.payload.validation_error())
            .or_else(|| {
                let mut seen: Vec<&str> = Vec::with_capacity(self.findings.len());
                let mut seen_workflow_steps: Vec<&str> = Vec::with_capacity(self.findings.len());
                for (index, finding) in self.findings.iter().enumerate() {
                    if let Some(error) = finding.validation_error() {
                        return Some(format!("review finding {index} is invalid: {error}"));
                    }
                    if seen
                        .iter()
                        .any(|finding_id| *finding_id == finding.finding_id)
                    {
                        return Some(format!(
                            "review finding {index} reuses duplicate finding_id {}",
                            finding.finding_id
                        ));
                    }
                    if let Some(error) = validate_review_finding_matches_run(&self.payload, finding)
                    {
                        return Some(format!("review finding {index} is invalid: {error}"));
                    }
                    if let ReviewFindingPayload::WorkflowStep { step } = &finding.payload {
                        if seen_workflow_steps
                            .iter()
                            .any(|step_id| *step_id == step.step_id)
                        {
                            return Some(format!(
                                "review finding {index} reuses duplicate workflow step_id {}",
                                step.step_id
                            ));
                        }
                        seen_workflow_steps.push(step.step_id.as_str());
                    }
                    seen.push(finding.finding_id.as_str());
                }
                None
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRunSummary {
    #[serde(flatten)]
    pub metadata: ReviewRunMetadata,
    pub kind: ReviewRunKind,
    pub finding_count: usize,
    pub status_counts: ReviewFindingStatusCounts,
}

impl From<&ReviewRun> for ReviewRunSummary {
    fn from(review: &ReviewRun) -> Self {
        Self {
            metadata: review.metadata.clone(),
            kind: review.kind(),
            finding_count: review.findings.len(),
            status_counts: ReviewFindingStatusCounts::from_findings(&review.findings),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingPair {
    pub finding_id: String,
    pub base: ReviewFinding,
    pub target: ReviewFinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingStatusDiff {
    pub finding_id: String,
    pub from_status: ReviewFindingStatus,
    pub to_status: ReviewFindingStatus,
    pub base: ReviewFinding,
    pub target: ReviewFinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRunDiff {
    pub base_review: ReviewRunSummary,
    pub target_review: ReviewRunSummary,
    pub added: Vec<ReviewFinding>,
    pub removed: Vec<ReviewFinding>,
    pub unchanged: Vec<ReviewFindingPair>,
    pub content_changed: Vec<ReviewFindingPair>,
    pub status_changed: Vec<ReviewFindingStatusDiff>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewRunDiffBucket {
    Added,
    Removed,
    Unchanged,
    ContentChanged,
    StatusChanged,
}

impl ReviewRunDiffBucket {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Unchanged => "unchanged",
            Self::ContentChanged => "content-changed",
            Self::StatusChanged => "status-changed",
        }
    }
}

impl ReviewRunDiff {
    pub fn between(base: &ReviewRun, target: &ReviewRun) -> Result<Self, String> {
        if let Some(error) = base.validation_error() {
            return Err(format!("base review run is invalid: {error}"));
        }
        if let Some(error) = target.validation_error() {
            return Err(format!("target review run is invalid: {error}"));
        }
        validate_review_diff_compatibility(&base.payload, &target.payload)?;

        let base_findings = base
            .findings
            .iter()
            .map(|finding| (finding.finding_id.as_str(), finding))
            .collect::<BTreeMap<_, _>>();
        let target_findings = target
            .findings
            .iter()
            .map(|finding| (finding.finding_id.as_str(), finding))
            .collect::<BTreeMap<_, _>>();

        let mut added = Vec::new();
        let mut unchanged = Vec::new();
        let mut content_changed = Vec::new();
        let mut status_changed = Vec::new();
        for (finding_id, target_finding) in &target_findings {
            if let Some(base_finding) = base_findings.get(finding_id) {
                if *base_finding == *target_finding {
                    unchanged.push(ReviewFindingPair {
                        finding_id: (*finding_id).to_owned(),
                        base: (*base_finding).clone(),
                        target: (*target_finding).clone(),
                    });
                } else if base_finding.status != target_finding.status {
                    status_changed.push(ReviewFindingStatusDiff {
                        finding_id: (*finding_id).to_owned(),
                        from_status: base_finding.status,
                        to_status: target_finding.status,
                        base: (*base_finding).clone(),
                        target: (*target_finding).clone(),
                    });
                } else {
                    content_changed.push(ReviewFindingPair {
                        finding_id: (*finding_id).to_owned(),
                        base: (*base_finding).clone(),
                        target: (*target_finding).clone(),
                    });
                }
            } else {
                added.push((*target_finding).clone());
            }
        }

        let removed = base_findings
            .iter()
            .filter(|(finding_id, _)| !target_findings.contains_key(**finding_id))
            .map(|(_, finding)| (*finding).clone())
            .collect();

        Ok(Self {
            base_review: ReviewRunSummary::from(base),
            target_review: ReviewRunSummary::from(target),
            added,
            removed,
            unchanged,
            content_changed,
            status_changed,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportProfileMetadata {
    pub profile_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
}

impl ReportProfileMetadata {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_report_profile_id_field(&self.profile_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportProfileSubject {
    Review,
    Routine,
    Audit,
    Workflow,
    Diff,
}

impl ReportProfileSubject {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Routine => "routine",
            Self::Audit => "audit",
            Self::Workflow => "workflow",
            Self::Diff => "diff",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ReportProfileMode {
    Summary,
    #[default]
    Detail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportJsonlLineKind {
    Workflow,
    Step,
    Audit,
    Entry,
    Review,
    Finding,
    Routine,
    Diff,
    Added,
    Removed,
    Unchanged,
    ContentChanged,
    StatusChanged,
    Unsupported(String),
}

impl ReportJsonlLineKind {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Workflow => "workflow",
            Self::Step => "step",
            Self::Audit => "audit",
            Self::Entry => "entry",
            Self::Review => "review",
            Self::Finding => "finding",
            Self::Routine => "routine",
            Self::Diff => "diff",
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Unchanged => "unchanged",
            Self::ContentChanged => "content-changed",
            Self::StatusChanged => "status-changed",
            Self::Unsupported(value) => value.as_str(),
        }
    }

    #[must_use]
    pub fn from_label(value: &str) -> Self {
        match value {
            "workflow" => Self::Workflow,
            "step" => Self::Step,
            "audit" => Self::Audit,
            "entry" => Self::Entry,
            "review" => Self::Review,
            "finding" => Self::Finding,
            "routine" => Self::Routine,
            "diff" => Self::Diff,
            "added" => Self::Added,
            "removed" => Self::Removed,
            "unchanged" => Self::Unchanged,
            "content-changed" => Self::ContentChanged,
            "status-changed" => Self::StatusChanged,
            other => Self::Unsupported(other.to_owned()),
        }
    }

    #[must_use]
    pub const fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported(_))
    }

    #[must_use]
    pub const fn is_detail_line(&self) -> bool {
        matches!(
            self,
            Self::Step
                | Self::Entry
                | Self::Finding
                | Self::Added
                | Self::Removed
                | Self::Unchanged
                | Self::ContentChanged
                | Self::StatusChanged
        )
    }
}

impl Serialize for ReportJsonlLineKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.label())
    }
}

impl<'de> Deserialize<'de> for ReportJsonlLineKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from_label(&value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportProfileSpec {
    #[serde(flatten)]
    pub metadata: ReportProfileMetadata,
    #[serde(default)]
    pub subjects: Vec<ReportProfileSubject>,
    #[serde(default)]
    pub mode: ReportProfileMode,
    #[serde(default)]
    pub status_filters: Option<Vec<ReviewFindingStatus>>,
    #[serde(default)]
    pub diff_buckets: Option<Vec<ReviewRunDiffBucket>>,
    #[serde(default)]
    pub jsonl_line_kinds: Option<Vec<ReportJsonlLineKind>>,
}

impl ReportProfileSpec {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.metadata
            .validation_error()
            .or_else(|| validate_report_profile_subjects(&self.subjects))
            .or_else(|| validate_report_profile_status_filters(self))
            .or_else(|| validate_report_profile_diff_buckets(self))
            .or_else(|| validate_report_profile_jsonl_line_kinds(self))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReportProfileCatalog {
    #[serde(default)]
    pub profiles: Vec<ReportProfileSpec>,
}

impl ReportProfileCatalog {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        let mut seen: Vec<&str> = Vec::with_capacity(self.profiles.len());
        for (index, profile) in self.profiles.iter().enumerate() {
            if let Some(error) = profile.validation_error() {
                return Some(format!("report profile {index} is invalid: {error}"));
            }
            if seen.contains(&profile.metadata.profile_id.as_str()) {
                return Some(format!(
                    "report profile {index} reuses duplicate profile_id {}",
                    profile.metadata.profile_id
                ));
            }
            seen.push(profile.metadata.profile_id.as_str());
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoutineMetadata {
    pub routine_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
}

impl ReviewRoutineMetadata {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_routine_id_field(&self.routine_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewRoutineSourceKind {
    Audit,
    Workflow,
    Unsupported,
}

impl ReviewRoutineSourceKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Audit => "audit",
            Self::Workflow => "workflow",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReviewRoutineSource {
    Audit {
        audit: CorpusAuditKind,
        #[serde(default = "default_audit_limit")]
        limit: usize,
    },
    Workflow {
        workflow_id: String,
    },
    #[serde(other)]
    Unsupported,
}

impl ReviewRoutineSource {
    #[must_use]
    pub const fn kind(&self) -> ReviewRoutineSourceKind {
        match self {
            Self::Audit { .. } => ReviewRoutineSourceKind::Audit,
            Self::Workflow { .. } => ReviewRoutineSourceKind::Workflow,
            Self::Unsupported => ReviewRoutineSourceKind::Unsupported,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::Audit { .. } => None,
            Self::Workflow { workflow_id } => validate_workflow_id_field(workflow_id),
            Self::Unsupported => Some("review routine source kind is unsupported".to_owned()),
        }
    }
}

#[must_use]
pub const fn default_review_routine_save_review_enabled() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoutineSaveReviewPolicy {
    #[serde(default = "default_review_routine_save_review_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub review_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
}

impl Default for ReviewRoutineSaveReviewPolicy {
    fn default() -> Self {
        Self {
            enabled: default_review_routine_save_review_enabled(),
            review_id: None,
            title: None,
            summary: None,
            overwrite: false,
        }
    }
}

impl ReviewRoutineSaveReviewPolicy {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_optional_review_id_field(self.review_id.as_deref(), "review_id")
            .or_else(|| validate_optional_text_field(self.title.as_deref(), "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
            .or_else(|| {
                (!self.enabled
                    && (self.review_id.is_some()
                        || self.title.is_some()
                        || self.summary.is_some()
                        || self.overwrite))
                .then(|| {
                    "disabled save_review policy cannot set review_id, title, summary, or overwrite"
                        .to_owned()
                })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewRoutineCompareTarget {
    #[default]
    LatestCompatibleReview,
    #[serde(other)]
    Unsupported,
}

impl ReviewRoutineCompareTarget {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LatestCompatibleReview => "latest-compatible-review",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoutineComparePolicy {
    #[serde(default)]
    pub target: ReviewRoutineCompareTarget,
    #[serde(default)]
    pub report_profile_id: Option<String>,
}

impl ReviewRoutineComparePolicy {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        matches!(self.target, ReviewRoutineCompareTarget::Unsupported)
            .then(|| "review routine compare target is unsupported".to_owned())
            .or_else(|| {
                validate_optional_report_profile_id_field(
                    self.report_profile_id.as_deref(),
                    "report_profile_id",
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoutineSpec {
    #[serde(flatten)]
    pub metadata: ReviewRoutineMetadata,
    pub source: ReviewRoutineSource,
    #[serde(default)]
    pub inputs: Vec<WorkflowInputSpec>,
    #[serde(default)]
    pub save_review: ReviewRoutineSaveReviewPolicy,
    #[serde(default)]
    pub compare: Option<ReviewRoutineComparePolicy>,
    #[serde(default)]
    pub report_profile_ids: Vec<String>,
}

impl ReviewRoutineSpec {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.metadata
            .validation_error()
            .or_else(|| self.source.validation_error())
            .or_else(|| validate_review_routine_inputs(self))
            .or_else(|| self.save_review.validation_error())
            .or_else(|| validate_review_routine_compare_policy(self))
            .or_else(|| validate_review_routine_report_profiles(&self.report_profile_ids))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReviewRoutineCatalog {
    #[serde(default)]
    pub routines: Vec<ReviewRoutineSpec>,
}

impl ReviewRoutineCatalog {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        let mut seen: Vec<&str> = Vec::with_capacity(self.routines.len());
        for (index, routine) in self.routines.iter().enumerate() {
            if let Some(error) = routine.validation_error() {
                return Some(format!("review routine {index} is invalid: {error}"));
            }
            if seen.contains(&routine.metadata.routine_id.as_str()) {
                return Some(format!(
                    "review routine {index} reuses duplicate routine_id {}",
                    routine.metadata.routine_id
                ));
            }
            seen.push(routine.metadata.routine_id.as_str());
        }
        None
    }
}

pub const WORKBENCH_PACK_MANIFEST_VERSION: u32 = 1;

#[must_use]
pub const fn default_workbench_pack_manifest_version() -> u32 {
    WORKBENCH_PACK_MANIFEST_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbenchPackCompatibility {
    #[serde(default = "default_workbench_pack_manifest_version")]
    pub version: u32,
}

impl Default for WorkbenchPackCompatibility {
    fn default() -> Self {
        Self {
            version: WORKBENCH_PACK_MANIFEST_VERSION,
        }
    }
}

impl WorkbenchPackCompatibility {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        if self.version == 0 {
            return Some(
                "workbench pack compatibility version must be greater than zero".to_owned(),
            );
        }
        (self.version > WORKBENCH_PACK_MANIFEST_VERSION).then(|| {
            format!(
                "unsupported workbench pack compatibility version {}; supported version is {}",
                self.version, WORKBENCH_PACK_MANIFEST_VERSION
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbenchPackCompatibilityEnvelope {
    #[serde(default)]
    pub pack_id: Option<String>,
    #[serde(default)]
    pub compatibility: WorkbenchPackCompatibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbenchPackMetadata {
    pub pack_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
}

impl WorkbenchPackMetadata {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workbench_pack_id_field(&self.pack_id)
            .or_else(|| validate_required_text_field(&self.title, "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchPackIssueKind {
    InvalidMetadata,
    UnsupportedVersion,
    EmptyPack,
    InvalidWorkflow,
    InvalidReviewRoutine,
    InvalidReportProfile,
    DuplicateWorkflowId,
    DuplicateReviewRoutineId,
    DuplicateReportProfileId,
    DuplicateReviewRoutineReference,
    MissingWorkflowReference,
    MissingReviewRoutineReference,
    MissingReportProfileReference,
    InvalidReviewRoutineReference,
}

impl WorkbenchPackIssueKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InvalidMetadata => "invalid-metadata",
            Self::UnsupportedVersion => "unsupported-version",
            Self::EmptyPack => "empty-pack",
            Self::InvalidWorkflow => "invalid-workflow",
            Self::InvalidReviewRoutine => "invalid-review-routine",
            Self::InvalidReportProfile => "invalid-report-profile",
            Self::DuplicateWorkflowId => "duplicate-workflow-id",
            Self::DuplicateReviewRoutineId => "duplicate-review-routine-id",
            Self::DuplicateReportProfileId => "duplicate-report-profile-id",
            Self::DuplicateReviewRoutineReference => "duplicate-review-routine-reference",
            Self::MissingWorkflowReference => "missing-workflow-reference",
            Self::MissingReviewRoutineReference => "missing-review-routine-reference",
            Self::MissingReportProfileReference => "missing-report-profile-reference",
            Self::InvalidReviewRoutineReference => "invalid-review-routine-reference",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbenchPackIssue {
    pub kind: WorkbenchPackIssueKind,
    #[serde(default)]
    pub asset_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbenchPackSummary {
    #[serde(flatten)]
    pub metadata: WorkbenchPackMetadata,
    pub compatibility: WorkbenchPackCompatibility,
    pub workflow_count: usize,
    pub review_routine_count: usize,
    pub report_profile_count: usize,
    #[serde(default)]
    pub entrypoint_routine_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbenchPackManifest {
    #[serde(flatten)]
    pub metadata: WorkbenchPackMetadata,
    #[serde(default)]
    pub compatibility: WorkbenchPackCompatibility,
    #[serde(default)]
    pub workflows: Vec<WorkflowSpec>,
    #[serde(default)]
    pub review_routines: Vec<ReviewRoutineSpec>,
    #[serde(default)]
    pub report_profiles: Vec<ReportProfileSpec>,
    #[serde(default)]
    pub entrypoint_routine_ids: Vec<String>,
}

impl WorkbenchPackManifest {
    #[must_use]
    pub fn summary(&self) -> WorkbenchPackSummary {
        WorkbenchPackSummary::from(self)
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.validation_issues()
            .into_iter()
            .next()
            .map(|issue| issue.message)
    }

    #[must_use]
    pub fn validation_issues(&self) -> Vec<WorkbenchPackIssue> {
        validate_workbench_pack_manifest(self)
    }
}

impl From<&WorkbenchPackManifest> for WorkbenchPackSummary {
    fn from(manifest: &WorkbenchPackManifest) -> Self {
        Self {
            metadata: manifest.metadata.clone(),
            compatibility: manifest.compatibility,
            workflow_count: manifest.workflows.len(),
            review_routine_count: manifest.review_routines.len(),
            report_profile_count: manifest.report_profiles.len(),
            entrypoint_routine_ids: manifest.entrypoint_routine_ids.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbenchPackIdParams {
    pub pack_id: String,
}

impl WorkbenchPackIdParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workbench_pack_id_field(&self.pack_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportWorkbenchPackParams {
    pub pack: WorkbenchPackManifest,
    #[serde(default)]
    pub overwrite: bool,
}

impl ImportWorkbenchPackParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.pack.validation_error()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidateWorkbenchPackParams {
    pub pack: WorkbenchPackManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListWorkbenchPacksParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportWorkbenchPackResult {
    pub pack: WorkbenchPackSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbenchPackResult {
    pub pack: WorkbenchPackManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidateWorkbenchPackResult {
    #[serde(default)]
    pub pack: Option<WorkbenchPackSummary>,
    pub valid: bool,
    pub issues: Vec<WorkbenchPackIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListWorkbenchPacksResult {
    pub packs: Vec<WorkbenchPackSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteWorkbenchPackResult {
    pub pack_id: String,
}

fn validate_review_diff_compatibility(
    base: &ReviewRunPayload,
    target: &ReviewRunPayload,
) -> Result<(), String> {
    match (base, target) {
        (
            ReviewRunPayload::Audit {
                audit: base_audit,
                limit: base_limit,
            },
            ReviewRunPayload::Audit {
                audit: target_audit,
                limit: target_limit,
            },
        ) => {
            if base_audit != target_audit {
                return Err(format!(
                    "cannot diff audit reviews with different audit kinds: {:?} vs {:?}",
                    base_audit, target_audit
                ));
            }
            if base_limit != target_limit {
                return Err(format!(
                    "cannot diff audit reviews with different limits: {base_limit} vs {target_limit}"
                ));
            }
            Ok(())
        }
        (
            ReviewRunPayload::Workflow {
                workflow: base_workflow,
                inputs: base_inputs,
                step_ids: base_step_ids,
            },
            ReviewRunPayload::Workflow {
                workflow: target_workflow,
                inputs: target_inputs,
                step_ids: target_step_ids,
            },
        ) => {
            if base_workflow.metadata.workflow_id != target_workflow.metadata.workflow_id {
                return Err(format!(
                    "cannot diff workflow reviews with different workflow IDs: {} vs {}",
                    base_workflow.metadata.workflow_id, target_workflow.metadata.workflow_id
                ));
            }
            if base_workflow.step_count != target_workflow.step_count {
                return Err(format!(
                    "cannot diff workflow reviews with different step counts: {} vs {}",
                    base_workflow.step_count, target_workflow.step_count
                ));
            }
            if base_inputs != target_inputs {
                return Err("cannot diff workflow reviews with different inputs".to_owned());
            }
            if base_step_ids != target_step_ids {
                return Err(
                    "cannot diff workflow reviews with different source step IDs".to_owned(),
                );
            }
            Ok(())
        }
        (ReviewRunPayload::Audit { .. }, ReviewRunPayload::Workflow { .. })
        | (ReviewRunPayload::Workflow { .. }, ReviewRunPayload::Audit { .. }) => {
            Err("cannot diff review runs with different kinds".to_owned())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRunIdParams {
    pub review_id: String,
}

impl ReviewRunIdParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_id_field(&self.review_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRunDiffParams {
    pub base_review_id: String,
    pub target_review_id: String,
}

impl ReviewRunDiffParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_id_field(&self.base_review_id)
            .or_else(|| validate_review_id_field(&self.target_review_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingRemediationPreviewParams {
    pub review_id: String,
    pub finding_id: String,
}

impl ReviewFindingRemediationPreviewParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_id_field(&self.review_id)
            .or_else(|| validate_review_finding_id_field(&self.finding_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListReviewRunsParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveReviewRunParams {
    pub review: ReviewRun,
    #[serde(default = "default_review_overwrite")]
    pub overwrite: bool,
}

impl SaveReviewRunParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.review.validation_error()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkReviewFindingParams {
    pub review_id: String,
    pub finding_id: String,
    pub status: ReviewFindingStatus,
}

impl MarkReviewFindingParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_id_field(&self.review_id)
            .or_else(|| validate_review_finding_id_field(&self.finding_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveCorpusAuditReviewParams {
    pub audit: CorpusAuditKind,
    #[serde(default = "default_audit_limit")]
    pub limit: usize,
    #[serde(default)]
    pub review_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default = "default_review_overwrite")]
    pub overwrite: bool,
}

impl SaveCorpusAuditReviewParams {
    #[must_use]
    pub fn audit_params(&self) -> CorpusAuditParams {
        CorpusAuditParams {
            audit: self.audit,
            limit: self.limit,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.review_id
            .as_deref()
            .and_then(validate_review_id_field)
            .or_else(|| validate_optional_text_field(self.title.as_deref(), "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveWorkflowReviewParams {
    pub workflow_id: String,
    #[serde(default)]
    pub inputs: Vec<WorkflowInputAssignment>,
    #[serde(default)]
    pub review_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default = "default_review_overwrite")]
    pub overwrite: bool,
}

impl SaveWorkflowReviewParams {
    #[must_use]
    pub fn run_workflow_params(&self) -> RunWorkflowParams {
        RunWorkflowParams {
            workflow_id: self.workflow_id.clone(),
            inputs: self.inputs.clone(),
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_workflow_id_field(&self.workflow_id)
            .or_else(|| {
                let mut seen: Vec<&str> = Vec::with_capacity(self.inputs.len());
                for (index, input) in self.inputs.iter().enumerate() {
                    if let Some(error) = input.validation_error() {
                        return Some(format!(
                            "workflow input assignment {index} is invalid: {error}"
                        ));
                    }
                    if seen.contains(&input.input_id.as_str()) {
                        return Some(format!(
                            "workflow input assignment {index} reuses duplicate input_id {}",
                            input.input_id
                        ));
                    }
                    seen.push(input.input_id.as_str());
                }
                None
            })
            .or_else(|| self.review_id.as_deref().and_then(validate_review_id_field))
            .or_else(|| validate_optional_text_field(self.title.as_deref(), "title"))
            .or_else(|| validate_optional_text_field(self.summary.as_deref(), "summary"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveReviewRunResult {
    pub review: ReviewRunSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveCorpusAuditReviewResult {
    pub result: CorpusAuditResult,
    pub review: ReviewRunSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveWorkflowReviewResult {
    pub result: WorkflowExecutionResult,
    pub review: ReviewRunSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRunResult {
    pub review: ReviewRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRunDiffResult {
    pub diff: ReviewRunDiff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingRemediationPreviewResult {
    pub preview: ReviewFindingRemediationPreview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListReviewRunsResult {
    pub reviews: Vec<ReviewRunSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteReviewRunResult {
    pub review_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkReviewFindingResult {
    pub transition: ReviewFindingStatusTransition,
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

const fn default_artifact_overwrite() -> bool {
    true
}

const fn default_review_overwrite() -> bool {
    true
}

const fn default_audit_limit() -> usize {
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

fn validate_workflow_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "workflow_id").or_else(|| {
        (value.trim() != value)
            .then(|| "workflow_id must not have leading or trailing whitespace".to_owned())
    })
}

fn validate_workflow_input_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "input_id").or_else(|| {
        (value.trim() != value)
            .then(|| "input_id must not have leading or trailing whitespace".to_owned())
    })
}

fn validate_workflow_step_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "step_id").or_else(|| {
        (value.trim() != value)
            .then(|| "step_id must not have leading or trailing whitespace".to_owned())
    })
}

fn validate_review_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "review_id").or_else(|| {
        (value.trim() != value)
            .then(|| "review_id must not have leading or trailing whitespace".to_owned())
    })
}

fn validate_optional_review_id_field(value: Option<&str>, field: &str) -> Option<String> {
    value.and_then(|review_id| {
        validate_required_text_field(review_id, field).or_else(|| {
            (review_id.trim() != review_id)
                .then(|| format!("{field} must not have leading or trailing whitespace"))
        })
    })
}

fn validate_review_finding_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "finding_id").or_else(|| {
        (value.trim() != value)
            .then(|| "finding_id must not have leading or trailing whitespace".to_owned())
    })
}

fn validate_review_routine_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "routine_id").or_else(|| {
        (value.trim() != value)
            .then(|| "routine_id must not have leading or trailing whitespace".to_owned())
    })
}

fn validate_workbench_pack_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "pack_id").or_else(|| {
        (value.trim() != value)
            .then(|| "pack_id must not have leading or trailing whitespace".to_owned())
    })
}

fn validate_report_profile_id_field(value: &str) -> Option<String> {
    validate_required_text_field(value, "profile_id").or_else(|| {
        (value.trim() != value)
            .then(|| "profile_id must not have leading or trailing whitespace".to_owned())
    })
}

fn validate_optional_report_profile_id_field(value: Option<&str>, field: &str) -> Option<String> {
    value.and_then(|profile_id| {
        validate_required_text_field(profile_id, field).or_else(|| {
            (profile_id.trim() != profile_id)
                .then(|| format!("{field} must not have leading or trailing whitespace"))
        })
    })
}

fn validate_optional_text_field(value: Option<&str>, field: &str) -> Option<String> {
    value.and_then(|text| validate_required_text_field(text, field))
}

fn validate_report_profile_subjects(subjects: &[ReportProfileSubject]) -> Option<String> {
    if subjects.is_empty() {
        return Some("report profiles must select at least one subject".to_owned());
    }

    let mut seen: Vec<ReportProfileSubject> = Vec::with_capacity(subjects.len());
    for (index, subject) in subjects.iter().copied().enumerate() {
        if seen.contains(&subject) {
            return Some(format!(
                "report profile subject {index} is duplicate: {}",
                subject.label()
            ));
        }
        seen.push(subject);
    }
    None
}

fn validate_report_profile_status_filters(profile: &ReportProfileSpec) -> Option<String> {
    let Some(status_filters) = &profile.status_filters else {
        return None;
    };
    if status_filters.is_empty() {
        return Some("report profile status_filters must not be empty when present".to_owned());
    }
    if !profile
        .subjects
        .iter()
        .any(|subject| report_profile_subject_supports_status_filters(*subject))
    {
        return Some(
            "report profile status_filters require a review, routine, or diff subject".to_owned(),
        );
    }

    let mut seen: Vec<ReviewFindingStatus> = Vec::with_capacity(status_filters.len());
    for (index, status) in status_filters.iter().copied().enumerate() {
        if seen.contains(&status) {
            return Some(format!(
                "report profile status_filters entry {index} is duplicate: {}",
                status.label()
            ));
        }
        seen.push(status);
    }
    None
}

fn validate_report_profile_diff_buckets(profile: &ReportProfileSpec) -> Option<String> {
    let Some(diff_buckets) = &profile.diff_buckets else {
        return None;
    };
    if diff_buckets.is_empty() {
        return Some("report profile diff_buckets must not be empty when present".to_owned());
    }
    if !profile.subjects.contains(&ReportProfileSubject::Diff) {
        return Some("report profile diff_buckets require a diff subject".to_owned());
    }

    let mut seen: Vec<ReviewRunDiffBucket> = Vec::with_capacity(diff_buckets.len());
    for (index, bucket) in diff_buckets.iter().copied().enumerate() {
        if seen.contains(&bucket) {
            return Some(format!(
                "report profile diff_buckets entry {index} is duplicate: {}",
                bucket.label()
            ));
        }
        seen.push(bucket);
    }
    None
}

fn validate_report_profile_jsonl_line_kinds(profile: &ReportProfileSpec) -> Option<String> {
    let Some(line_kinds) = &profile.jsonl_line_kinds else {
        return None;
    };
    if line_kinds.is_empty() {
        return Some("report profile jsonl_line_kinds must not be empty when present".to_owned());
    }

    let mut seen: Vec<&str> = Vec::with_capacity(line_kinds.len());
    for (index, line_kind) in line_kinds.iter().enumerate() {
        let label = line_kind.label();
        if label.trim().is_empty() {
            return Some(format!(
                "report profile jsonl_line_kinds entry {index} must not be empty"
            ));
        }
        if !line_kind.is_supported() {
            return Some(format!(
                "report profile jsonl_line_kinds entry {index} is unsupported: {label}"
            ));
        }
        if seen.contains(&label) {
            return Some(format!(
                "report profile jsonl_line_kinds entry {index} is duplicate: {label}"
            ));
        }
        if matches!(profile.mode, ReportProfileMode::Summary) && line_kind.is_detail_line() {
            return Some(format!(
                "report profile summary mode cannot select detail JSONL line kind: {label}"
            ));
        }
        if !profile
            .subjects
            .iter()
            .any(|subject| report_profile_subject_supports_line_kind(*subject, line_kind))
        {
            return Some(format!(
                "report profile jsonl_line_kinds entry {index} is not supported by selected subjects: {label}"
            ));
        }
        seen.push(label);
    }
    None
}

const fn report_profile_subject_supports_status_filters(subject: ReportProfileSubject) -> bool {
    matches!(
        subject,
        ReportProfileSubject::Review | ReportProfileSubject::Routine | ReportProfileSubject::Diff
    )
}

const fn report_profile_subject_supports_line_kind(
    subject: ReportProfileSubject,
    line_kind: &ReportJsonlLineKind,
) -> bool {
    match subject {
        ReportProfileSubject::Workflow => {
            matches!(
                line_kind,
                ReportJsonlLineKind::Workflow | ReportJsonlLineKind::Step
            )
        }
        ReportProfileSubject::Audit => {
            matches!(
                line_kind,
                ReportJsonlLineKind::Audit | ReportJsonlLineKind::Entry
            )
        }
        ReportProfileSubject::Review => matches!(
            line_kind,
            ReportJsonlLineKind::Review | ReportJsonlLineKind::Finding
        ),
        ReportProfileSubject::Routine => matches!(
            line_kind,
            ReportJsonlLineKind::Routine
                | ReportJsonlLineKind::Step
                | ReportJsonlLineKind::Review
                | ReportJsonlLineKind::Finding
        ),
        ReportProfileSubject::Diff => matches!(
            line_kind,
            ReportJsonlLineKind::Diff
                | ReportJsonlLineKind::Added
                | ReportJsonlLineKind::Removed
                | ReportJsonlLineKind::Unchanged
                | ReportJsonlLineKind::ContentChanged
                | ReportJsonlLineKind::StatusChanged
        ),
    }
}

fn validate_review_routine_inputs(routine: &ReviewRoutineSpec) -> Option<String> {
    if matches!(routine.source, ReviewRoutineSource::Audit { .. }) && !routine.inputs.is_empty() {
        return Some("audit review routines cannot declare workflow inputs".to_owned());
    }

    let mut seen: Vec<&str> = Vec::with_capacity(routine.inputs.len());
    for (index, input) in routine.inputs.iter().enumerate() {
        if let Some(error) = input.validation_error() {
            return Some(format!("review routine input {index} is invalid: {error}"));
        }
        if seen.contains(&input.input_id.as_str()) {
            return Some(format!(
                "review routine input {index} reuses duplicate input_id {}",
                input.input_id
            ));
        }
        seen.push(input.input_id.as_str());
    }
    None
}

fn validate_review_routine_compare_policy(routine: &ReviewRoutineSpec) -> Option<String> {
    let Some(compare) = &routine.compare else {
        return None;
    };
    if !routine.save_review.enabled {
        return Some("review routine compare policy requires save_review to be enabled".to_owned());
    }
    compare.validation_error()
}

fn validate_review_routine_report_profiles(report_profile_ids: &[String]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::with_capacity(report_profile_ids.len());
    for (index, profile_id) in report_profile_ids.iter().enumerate() {
        if let Some(error) = validate_report_profile_id_field(profile_id) {
            return Some(format!(
                "review routine report_profile_ids entry {index} is invalid: {error}"
            ));
        }
        if seen.contains(&profile_id.as_str()) {
            return Some(format!(
                "review routine report_profile_ids entry {index} is duplicate: {profile_id}"
            ));
        }
        seen.push(profile_id.as_str());
    }
    None
}

fn validate_workbench_pack_manifest(manifest: &WorkbenchPackManifest) -> Vec<WorkbenchPackIssue> {
    let mut issues = Vec::new();

    if let Some(error) = manifest.metadata.validation_error() {
        issues.push(workbench_pack_issue(
            WorkbenchPackIssueKind::InvalidMetadata,
            Some(manifest.metadata.pack_id.clone()),
            error,
        ));
    }
    if let Some(error) = manifest.compatibility.validation_error() {
        issues.push(workbench_pack_issue(
            WorkbenchPackIssueKind::UnsupportedVersion,
            Some(manifest.metadata.pack_id.clone()),
            error,
        ));
    }
    if manifest.workflows.is_empty()
        && manifest.review_routines.is_empty()
        && manifest.report_profiles.is_empty()
    {
        issues.push(workbench_pack_issue(
            WorkbenchPackIssueKind::EmptyPack,
            Some(manifest.metadata.pack_id.clone()),
            "workbench packs must contain at least one workflow, review routine, or report profile",
        ));
    }

    let built_in_workflows = built_in_workflows();
    let mut workflow_inputs: Vec<(&str, &[WorkflowInputSpec])> = built_in_workflows
        .iter()
        .map(|workflow| {
            (
                workflow.metadata.workflow_id.as_str(),
                workflow.inputs.as_slice(),
            )
        })
        .collect();
    let mut workflow_ids = Vec::with_capacity(manifest.workflows.len());
    for (index, workflow) in manifest.workflows.iter().enumerate() {
        let collides_with_built_in = built_in_workflows
            .iter()
            .any(|built_in| built_in.metadata.workflow_id == workflow.metadata.workflow_id);
        if let Some(error) = workflow.validation_error() {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::InvalidWorkflow,
                Some(workflow.metadata.workflow_id.clone()),
                format!("workbench pack workflow {index} is invalid: {error}"),
            ));
        } else if !collides_with_built_in {
            workflow_inputs.push((
                workflow.metadata.workflow_id.as_str(),
                workflow.inputs.as_slice(),
            ));
        }
        if collides_with_built_in {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateWorkflowId,
                Some(workflow.metadata.workflow_id.clone()),
                format!(
                    "workbench pack workflow {index} collides with built-in workflow_id {}",
                    workflow.metadata.workflow_id
                ),
            ));
        }
        if workflow_ids.contains(&workflow.metadata.workflow_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateWorkflowId,
                Some(workflow.metadata.workflow_id.clone()),
                format!(
                    "workbench pack workflow {index} reuses duplicate workflow_id {}",
                    workflow.metadata.workflow_id
                ),
            ));
        }
        workflow_ids.push(workflow.metadata.workflow_id.as_str());
    }

    let mut report_profile_ids = Vec::with_capacity(manifest.report_profiles.len());
    for (index, profile) in manifest.report_profiles.iter().enumerate() {
        if let Some(error) = profile.validation_error() {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::InvalidReportProfile,
                Some(profile.metadata.profile_id.clone()),
                format!("workbench pack report profile {index} is invalid: {error}"),
            ));
        }
        if report_profile_ids.contains(&profile.metadata.profile_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateReportProfileId,
                Some(profile.metadata.profile_id.clone()),
                format!(
                    "workbench pack report profile {index} reuses duplicate profile_id {}",
                    profile.metadata.profile_id
                ),
            ));
        }
        report_profile_ids.push(profile.metadata.profile_id.as_str());
    }

    let mut routine_ids = Vec::with_capacity(manifest.review_routines.len());
    for (index, routine) in manifest.review_routines.iter().enumerate() {
        if let Some(error) = routine.validation_error() {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::InvalidReviewRoutine,
                Some(routine.metadata.routine_id.clone()),
                format!("workbench pack review routine {index} is invalid: {error}"),
            ));
        }
        if routine_ids.contains(&routine.metadata.routine_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateReviewRoutineId,
                Some(routine.metadata.routine_id.clone()),
                format!(
                    "workbench pack review routine {index} reuses duplicate routine_id {}",
                    routine.metadata.routine_id
                ),
            ));
        }
        validate_workbench_pack_routine_references(
            routine,
            &workflow_inputs,
            &report_profile_ids,
            &mut issues,
        );
        routine_ids.push(routine.metadata.routine_id.as_str());
    }

    validate_workbench_pack_entrypoint_routine_references(
        &manifest.entrypoint_routine_ids,
        &routine_ids,
        &mut issues,
    );

    issues
}

fn workbench_pack_issue(
    kind: WorkbenchPackIssueKind,
    asset_id: Option<String>,
    message: impl Into<String>,
) -> WorkbenchPackIssue {
    WorkbenchPackIssue {
        kind,
        asset_id,
        message: message.into(),
    }
}

fn validate_workbench_pack_routine_references(
    routine: &ReviewRoutineSpec,
    workflow_inputs: &[(&str, &[WorkflowInputSpec])],
    report_profile_ids: &[&str],
    issues: &mut Vec<WorkbenchPackIssue>,
) {
    if let ReviewRoutineSource::Workflow { workflow_id } = &routine.source {
        match workflow_inputs
            .iter()
            .find(|(candidate_id, _)| *candidate_id == workflow_id.as_str())
            .map(|(_, inputs)| *inputs)
        {
            Some(inputs) => {
                if let Some(error) = validate_workbench_pack_routine_inputs(routine, inputs) {
                    issues.push(workbench_pack_issue(
                        WorkbenchPackIssueKind::InvalidReviewRoutineReference,
                        Some(routine.metadata.routine_id.clone()),
                        error,
                    ));
                }
            }
            None => issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::MissingWorkflowReference,
                Some(routine.metadata.routine_id.clone()),
                format!(
                    "review routine {} references missing workflow_id {workflow_id}",
                    routine.metadata.routine_id
                ),
            )),
        }
    }

    if let Some(compare) = &routine.compare {
        if let Some(profile_id) = &compare.report_profile_id {
            validate_workbench_pack_report_profile_reference(
                &routine.metadata.routine_id,
                profile_id,
                report_profile_ids,
                issues,
            );
        }
    }
    for profile_id in &routine.report_profile_ids {
        validate_workbench_pack_report_profile_reference(
            &routine.metadata.routine_id,
            profile_id,
            report_profile_ids,
            issues,
        );
    }
}

fn validate_workbench_pack_routine_inputs(
    routine: &ReviewRoutineSpec,
    workflow_inputs: &[WorkflowInputSpec],
) -> Option<String> {
    for input in &routine.inputs {
        let Some(workflow_input) = workflow_inputs
            .iter()
            .find(|workflow_input| workflow_input.input_id == input.input_id)
        else {
            return Some(format!(
                "review routine {} declares input_id {} that referenced workflow does not accept",
                routine.metadata.routine_id, input.input_id
            ));
        };
        if workflow_input.kind != input.kind {
            return Some(format!(
                "review routine {} declares input_id {} as {}, but referenced workflow requires {}",
                routine.metadata.routine_id,
                input.input_id,
                input.kind.label(),
                workflow_input.kind.label()
            ));
        }
    }

    workflow_inputs
        .iter()
        .find(|workflow_input| {
            !routine
                .inputs
                .iter()
                .any(|input| input.input_id == workflow_input.input_id)
        })
        .map(|workflow_input| {
            format!(
                "review routine {} is missing input_id {} required by referenced workflow",
                routine.metadata.routine_id, workflow_input.input_id
            )
        })
}

fn validate_workbench_pack_report_profile_reference(
    routine_id: &str,
    profile_id: &str,
    report_profile_ids: &[&str],
    issues: &mut Vec<WorkbenchPackIssue>,
) {
    if !report_profile_ids.contains(&profile_id) {
        issues.push(workbench_pack_issue(
            WorkbenchPackIssueKind::MissingReportProfileReference,
            Some(routine_id.to_owned()),
            format!("review routine {routine_id} references missing profile_id {profile_id}"),
        ));
    }
}

fn validate_workbench_pack_entrypoint_routine_references(
    entrypoint_routine_ids: &[String],
    routine_ids: &[&str],
    issues: &mut Vec<WorkbenchPackIssue>,
) {
    let mut seen = Vec::with_capacity(entrypoint_routine_ids.len());
    for (index, routine_id) in entrypoint_routine_ids.iter().enumerate() {
        if let Some(error) = validate_review_routine_id_field(routine_id) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::InvalidReviewRoutineReference,
                Some(routine_id.clone()),
                format!("workbench pack entrypoint_routine_ids entry {index} is invalid: {error}"),
            ));
            continue;
        }
        if seen.contains(&routine_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::DuplicateReviewRoutineReference,
                Some(routine_id.clone()),
                format!(
                    "workbench pack entrypoint_routine_ids entry {index} is duplicate: {routine_id}"
                ),
            ));
        }
        if !routine_ids.contains(&routine_id.as_str()) {
            issues.push(workbench_pack_issue(
                WorkbenchPackIssueKind::MissingReviewRoutineReference,
                Some(routine_id.clone()),
                format!(
                    "workbench pack entrypoint_routine_ids entry {index} references missing routine_id {routine_id}"
                ),
            ));
        }
        seen.push(routine_id.as_str());
    }
}

fn workflow_step_kind_for_reference(
    seen: &[(&str, WorkflowStepKind)],
    step_id: &str,
) -> Option<WorkflowStepKind> {
    seen.iter()
        .find_map(|(existing_id, kind)| (*existing_id == step_id).then_some(*kind))
}

fn validate_workflow_step_reference(
    seen: &[(&str, WorkflowStepKind)],
    step_id: &str,
    expected: WorkflowStepKind,
    field: &str,
) -> Option<String> {
    match workflow_step_kind_for_reference(seen, step_id) {
        Some(kind) if kind == expected => None,
        Some(kind) => Some(format!(
            "{field} must reference a {} step, not {}",
            expected.label(),
            kind.label()
        )),
        None => Some(format!(
            "{field} must reference an earlier {} step",
            expected.label()
        )),
    }
}

fn workflow_input_kind_for_reference(
    inputs: &[(&str, WorkflowInputKind)],
    input_id: &str,
) -> Option<WorkflowInputKind> {
    inputs
        .iter()
        .find_map(|(existing_id, kind)| (*existing_id == input_id).then_some(*kind))
}

fn validate_workflow_input_reference_kind(
    inputs: &[(&str, WorkflowInputKind)],
    input_id: &str,
    required_kind: WorkflowInputKind,
    role: &str,
) -> Option<String> {
    match workflow_input_kind_for_reference(inputs, input_id) {
        None => Some(format!("{role} must reference a declared workflow input")),
        Some(kind) if kind != required_kind => Some(format!(
            "{role} must reference a declared {} input",
            match required_kind {
                WorkflowInputKind::NoteTarget => "note-target",
                WorkflowInputKind::FocusTarget => "focus-target",
            }
        )),
        Some(_) => None,
    }
}

fn validate_workflow_step_references(
    payload: &WorkflowStepPayload,
    seen: &[(&str, WorkflowStepKind)],
    inputs: &[(&str, WorkflowInputKind)],
) -> Option<String> {
    match payload {
        WorkflowStepPayload::Resolve { target } => match target {
            WorkflowResolveTarget::Input { input_id } => {
                (workflow_input_kind_for_reference(inputs, input_id).is_none())
                    .then(|| "target must reference a declared workflow input".to_owned())
            }
            WorkflowResolveTarget::Id { .. }
            | WorkflowResolveTarget::Title { .. }
            | WorkflowResolveTarget::Reference { .. }
            | WorkflowResolveTarget::NodeKey { .. } => None,
        },
        WorkflowStepPayload::ArtifactRun { .. } => None,
        WorkflowStepPayload::Explore { focus, .. } => match focus {
            WorkflowExploreFocus::NodeKey { .. } => None,
            WorkflowExploreFocus::Input { input_id } => validate_workflow_input_reference_kind(
                inputs,
                input_id,
                WorkflowInputKind::FocusTarget,
                "focus",
            ),
            WorkflowExploreFocus::ResolvedStep { step_id } => {
                validate_workflow_step_reference(seen, step_id, WorkflowStepKind::Resolve, "focus")
            }
        },
        WorkflowStepPayload::Compare { left, right, .. } => {
            validate_workflow_step_reference(seen, &left.step_id, WorkflowStepKind::Resolve, "left")
                .or_else(|| {
                    validate_workflow_step_reference(
                        seen,
                        &right.step_id,
                        WorkflowStepKind::Resolve,
                        "right",
                    )
                })
        }
        WorkflowStepPayload::ArtifactSave { source, .. } => match source {
            WorkflowArtifactSaveSource::ExploreStep { step_id } => {
                validate_workflow_step_reference(seen, step_id, WorkflowStepKind::Explore, "source")
            }
            WorkflowArtifactSaveSource::CompareStep { step_id } => {
                validate_workflow_step_reference(seen, step_id, WorkflowStepKind::Compare, "source")
            }
        },
    }
}

fn validate_workflow_review_source(
    workflow: &WorkflowSummary,
    step_ids: &[String],
) -> Option<String> {
    if workflow.step_count == 0 {
        return Some("workflow review source must contain at least one step".to_owned());
    }
    if step_ids.len() != workflow.step_count {
        return Some("workflow review source step_ids must match workflow step_count".to_owned());
    }

    let mut seen: Vec<&str> = Vec::with_capacity(step_ids.len());
    for (index, step_id) in step_ids.iter().enumerate() {
        if let Some(error) = validate_workflow_step_id_field(step_id) {
            return Some(format!(
                "workflow review source step_id {index} is invalid: {error}"
            ));
        }
        if seen.iter().any(|existing| *existing == step_id) {
            return Some(format!(
                "workflow review source step_id {index} reuses duplicate step_id {step_id}"
            ));
        }
        seen.push(step_id.as_str());
    }

    None
}

fn validate_workflow_review_inputs(inputs: &[WorkflowInputAssignment]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.iter().enumerate() {
        if let Some(error) = input.validation_error() {
            return Some(format!("workflow review input {index} is invalid: {error}"));
        }
        if seen.contains(&input.input_id.as_str()) {
            return Some(format!(
                "workflow review input {index} reuses duplicate input_id {}",
                input.input_id
            ));
        }
        seen.push(input.input_id.as_str());
    }
    None
}

fn validate_review_finding_matches_run(
    payload: &ReviewRunPayload,
    finding: &ReviewFinding,
) -> Option<String> {
    match (payload, &finding.payload) {
        (ReviewRunPayload::Audit { audit, .. }, ReviewFindingPayload::Audit { entry }) => {
            (entry.kind() != *audit)
                .then(|| "audit review findings must match review audit kind".to_owned())
        }
        (
            ReviewRunPayload::Workflow { step_ids, .. },
            ReviewFindingPayload::WorkflowStep { step },
        ) => (!step_ids.iter().any(|step_id| step_id == &step.step_id))
            .then(|| "workflow-step findings must reference a source workflow step".to_owned()),
        (ReviewRunPayload::Audit { .. }, ReviewFindingPayload::WorkflowStep { .. }) => {
            Some("audit review runs cannot contain workflow-step findings".to_owned())
        }
        (ReviewRunPayload::Workflow { .. }, ReviewFindingPayload::Audit { .. }) => {
            Some("workflow review runs cannot contain audit findings".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AnchorRecord, BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID, BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
        BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID, BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
        BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID, BacklinkRecord, BridgeEvidenceRecord,
        CaptureNodeParams, CaptureTemplatePreviewResult, CompareNotesParams,
        ComparisonConnectorDirection, ComparisonPlanningRecord, ComparisonReferenceRecord,
        ComparisonTaskStateRecord, CorpusAuditEntry, CorpusAuditKind, CorpusAuditParams,
        CorpusAuditReportLine, CorpusAuditResult, DanglingLinkAuditRecord,
        DeleteExplorationArtifactResult, DeleteWorkbenchPackResult, DuplicateTitleAuditRecord,
        ExecuteExplorationArtifactResult, ExecutedExplorationArtifact,
        ExecutedExplorationArtifactPayload, ExplorationArtifactIdParams, ExplorationArtifactKind,
        ExplorationArtifactMetadata, ExplorationArtifactPayload, ExplorationArtifactResult,
        ExplorationArtifactSummary, ExplorationEntry, ExplorationExplanation, ExplorationLens,
        ExplorationSection, ExplorationSectionKind, ExploreParams, ExploreResult,
        ImportWorkbenchPackParams, ImportWorkbenchPackResult, ListExplorationArtifactsResult,
        ListWorkbenchPacksResult, MarkReviewFindingParams, NodeFromKeyParams,
        NodeFromTitleOrAliasParams, NodeKind, NodeRecord, NoteComparisonEntry,
        NoteComparisonExplanation, NoteComparisonGroup, NoteComparisonResult,
        NoteComparisonSection, NoteComparisonSectionKind, NoteConnectivityAuditRecord,
        PlanningField, PlanningRelationRecord, PreviewNodeRecord, ReportJsonlLineKind,
        ReportProfileCatalog, ReportProfileMetadata, ReportProfileMode, ReportProfileSpec,
        ReportProfileSubject, ReviewFinding, ReviewFindingPayload, ReviewFindingRemediationPreview,
        ReviewFindingRemediationPreviewParams, ReviewFindingRemediationPreviewResult,
        ReviewFindingStatus, ReviewFindingStatusTransition, ReviewRoutineCatalog,
        ReviewRoutineComparePolicy, ReviewRoutineCompareTarget, ReviewRoutineMetadata,
        ReviewRoutineSaveReviewPolicy, ReviewRoutineSource, ReviewRoutineSpec, ReviewRun,
        ReviewRunDiff, ReviewRunDiffBucket, ReviewRunDiffParams, ReviewRunDiffResult,
        ReviewRunIdParams, ReviewRunMetadata, ReviewRunPayload, ReviewRunResult, ReviewRunSummary,
        SaveCorpusAuditReviewParams, SaveCorpusAuditReviewResult, SaveExplorationArtifactParams,
        SaveExplorationArtifactResult, SaveReviewRunParams, SaveReviewRunResult,
        SaveWorkflowReviewParams, SaveWorkflowReviewResult, SavedComparisonArtifact,
        SavedExplorationArtifact, SavedLensViewArtifact, SavedTrailArtifact, SavedTrailStep,
        SearchNodesParams, SearchNodesSort, TrailReplayResult, TrailReplayStepResult,
        UnlinkedReferencesParams, UpdateNodeMetadataParams, ValidateWorkbenchPackParams,
        ValidateWorkbenchPackResult, WorkbenchPackCompatibility,
        WorkbenchPackCompatibilityEnvelope, WorkbenchPackIdParams, WorkbenchPackIssueKind,
        WorkbenchPackManifest, WorkbenchPackMetadata, WorkbenchPackResult, WorkbenchPackSummary,
        WorkflowArtifactSaveSource, WorkflowExecutionResult, WorkflowExploreFocus,
        WorkflowInputAssignment, WorkflowInputKind, WorkflowInputSpec, WorkflowMetadata,
        WorkflowReportLine, WorkflowResolveTarget, WorkflowSpec, WorkflowSpecCompatibility,
        WorkflowSpecCompatibilityEnvelope, WorkflowStepPayload, WorkflowStepRef,
        WorkflowStepReport, WorkflowStepReportPayload, WorkflowStepSpec, WorkflowSummary,
        built_in_workflow, built_in_workflow_summaries, built_in_workflows, normalize_reference,
    };
    use serde_json::json;

    fn sample_node(node_key: &str, title: &str) -> NodeRecord {
        NodeRecord {
            node_key: node_key.to_owned(),
            explicit_id: None,
            file_path: "sample.org".to_owned(),
            title: title.to_owned(),
            outline_path: title.to_owned(),
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
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        }
    }

    fn sample_anchor(node_key: &str, title: &str) -> AnchorRecord {
        AnchorRecord {
            node_key: node_key.to_owned(),
            explicit_id: None,
            file_path: "sample.org".to_owned(),
            title: title.to_owned(),
            outline_path: title.to_owned(),
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
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        }
    }

    fn sample_pack_workflow() -> WorkflowSpec {
        WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/pack/context-review".to_owned(),
                title: "Pack Context Review".to_owned(),
                summary: Some("Collect context for a reusable routine".to_owned()),
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: vec![WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus".to_owned(),
                summary: None,
                kind: WorkflowInputKind::FocusTarget,
            }],
            steps: vec![WorkflowStepSpec {
                step_id: "explore-context".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Bridges,
                    limit: 25,
                    unique: false,
                },
            }],
        }
    }

    fn sample_pack_report_profiles() -> Vec<ReportProfileSpec> {
        vec![
            ReportProfileSpec {
                metadata: ReportProfileMetadata {
                    profile_id: "profile/routine-detail".to_owned(),
                    title: "Routine Detail".to_owned(),
                    summary: None,
                },
                subjects: vec![ReportProfileSubject::Routine, ReportProfileSubject::Review],
                mode: ReportProfileMode::Detail,
                status_filters: Some(vec![ReviewFindingStatus::Open]),
                diff_buckets: None,
                jsonl_line_kinds: Some(vec![
                    ReportJsonlLineKind::Routine,
                    ReportJsonlLineKind::Review,
                    ReportJsonlLineKind::Finding,
                ]),
            },
            ReportProfileSpec {
                metadata: ReportProfileMetadata {
                    profile_id: "profile/diff-focus".to_owned(),
                    title: "Diff Focus".to_owned(),
                    summary: None,
                },
                subjects: vec![ReportProfileSubject::Diff],
                mode: ReportProfileMode::Detail,
                status_filters: None,
                diff_buckets: Some(vec![
                    ReviewRunDiffBucket::Added,
                    ReviewRunDiffBucket::ContentChanged,
                ]),
                jsonl_line_kinds: Some(vec![
                    ReportJsonlLineKind::Diff,
                    ReportJsonlLineKind::Added,
                    ReportJsonlLineKind::ContentChanged,
                ]),
            },
        ]
    }

    fn sample_pack_workflow_routine() -> ReviewRoutineSpec {
        ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: "routine/pack/context-review".to_owned(),
                title: "Pack Context Review".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Workflow {
                workflow_id: "workflow/pack/context-review".to_owned(),
            },
            inputs: vec![WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus".to_owned(),
                summary: None,
                kind: WorkflowInputKind::FocusTarget,
            }],
            save_review: ReviewRoutineSaveReviewPolicy::default(),
            compare: Some(ReviewRoutineComparePolicy {
                target: ReviewRoutineCompareTarget::LatestCompatibleReview,
                report_profile_id: Some("profile/diff-focus".to_owned()),
            }),
            report_profile_ids: vec!["profile/routine-detail".to_owned()],
        }
    }

    fn sample_pack_audit_routine() -> ReviewRoutineSpec {
        ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: "routine/pack/duplicate-title-review".to_owned(),
                title: "Duplicate Title Review".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Audit {
                audit: CorpusAuditKind::DuplicateTitles,
                limit: 100,
            },
            inputs: Vec::new(),
            save_review: ReviewRoutineSaveReviewPolicy::default(),
            compare: None,
            report_profile_ids: vec!["profile/routine-detail".to_owned()],
        }
    }

    fn sample_workbench_pack_manifest() -> WorkbenchPackManifest {
        WorkbenchPackManifest {
            metadata: WorkbenchPackMetadata {
                pack_id: "pack/research-review".to_owned(),
                title: "Research Review Pack".to_owned(),
                summary: Some("Reusable review routines and output profiles".to_owned()),
            },
            compatibility: WorkbenchPackCompatibility::default(),
            workflows: vec![sample_pack_workflow()],
            review_routines: vec![sample_pack_workflow_routine(), sample_pack_audit_routine()],
            report_profiles: sample_pack_report_profiles(),
            entrypoint_routine_ids: vec![
                "routine/pack/context-review".to_owned(),
                "routine/pack/duplicate-title-review".to_owned(),
            ],
        }
    }

    fn sample_dangling_finding(
        finding_id: &str,
        missing_explicit_id: &str,
        status: ReviewFindingStatus,
    ) -> ReviewFinding {
        ReviewFinding {
            finding_id: finding_id.to_owned(),
            status,
            payload: ReviewFindingPayload::Audit {
                entry: Box::new(CorpusAuditEntry::DanglingLink {
                    record: Box::new(DanglingLinkAuditRecord {
                        source: sample_anchor("heading:source.org:3", "Source Heading"),
                        missing_explicit_id: missing_explicit_id.to_owned(),
                        line: 12,
                        column: 7,
                        preview: format!("[[id:{missing_explicit_id}][Missing]]"),
                    }),
                }),
            },
        }
    }

    fn sample_dangling_review(review_id: &str, findings: Vec<ReviewFinding>) -> ReviewRun {
        ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: review_id.to_owned(),
                title: "Dangling Link Review".to_owned(),
                summary: Some("Review missing id links".to_owned()),
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::DanglingLinks,
                limit: 200,
            },
            findings,
        }
    }

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
            overwrite: false,
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
        assert_eq!(save_json["overwrite"], json!(false));

        let save_round_trip: SaveExplorationArtifactParams =
            serde_json::from_value(save_json).expect("save params should deserialize");
        assert_eq!(save_round_trip, save_params);

        let legacy_round_trip: SaveExplorationArtifactParams =
            serde_json::from_value(json!({ "artifact": artifact.clone() }))
                .expect("legacy save params should deserialize");
        assert!(legacy_round_trip.overwrite);
        assert_eq!(legacy_round_trip.artifact, artifact);

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
    fn workflow_specs_round_trip_and_compose_settled_headless_steps() {
        let workflow = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/research-routine".to_owned(),
                title: "Research Routine".to_owned(),
                summary: Some("Resolve, explore, compare, and save".to_owned()),
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![
                WorkflowStepSpec {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Id {
                            id: "focus-id".to_owned(),
                        },
                    },
                },
                WorkflowStepSpec {
                    step_id: "resolve-neighbor".to_owned(),
                    payload: WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Title {
                            title: "Neighbor".to_owned(),
                        },
                    },
                },
                WorkflowStepSpec {
                    step_id: "run-saved-context".to_owned(),
                    payload: WorkflowStepPayload::ArtifactRun {
                        artifact_id: "artifact/context".to_owned(),
                    },
                },
                WorkflowStepSpec {
                    step_id: "explore-dormant".to_owned(),
                    payload: WorkflowStepPayload::Explore {
                        focus: WorkflowExploreFocus::ResolvedStep {
                            step_id: "resolve-focus".to_owned(),
                        },
                        lens: ExplorationLens::Dormant,
                        limit: 0,
                        unique: false,
                    },
                },
                WorkflowStepSpec {
                    step_id: "explore-context".to_owned(),
                    payload: WorkflowStepPayload::Explore {
                        focus: WorkflowExploreFocus::ResolvedStep {
                            step_id: "resolve-neighbor".to_owned(),
                        },
                        lens: ExplorationLens::Refs,
                        limit: 25,
                        unique: false,
                    },
                },
                WorkflowStepSpec {
                    step_id: "compare-focus-neighbor".to_owned(),
                    payload: WorkflowStepPayload::Compare {
                        left: WorkflowStepRef {
                            step_id: "resolve-focus".to_owned(),
                        },
                        right: WorkflowStepRef {
                            step_id: "resolve-neighbor".to_owned(),
                        },
                        group: NoteComparisonGroup::Tension,
                        limit: 10,
                    },
                },
                WorkflowStepSpec {
                    step_id: "save-comparison".to_owned(),
                    payload: WorkflowStepPayload::ArtifactSave {
                        source: WorkflowArtifactSaveSource::CompareStep {
                            step_id: "compare-focus-neighbor".to_owned(),
                        },
                        metadata: ExplorationArtifactMetadata {
                            artifact_id: "artifact/focus-vs-neighbor".to_owned(),
                            title: "Focus vs Neighbor".to_owned(),
                            summary: Some("Pinned comparison".to_owned()),
                        },
                        overwrite: false,
                    },
                },
            ],
        };

        assert_eq!(workflow.validation_error(), None);
        assert_eq!(WorkflowSummary::from(&workflow).step_count, 7);

        let serialized = serde_json::to_value(&workflow).expect("workflow spec should serialize");
        assert_eq!(
            serialized["workflow_id"],
            json!("workflow/research-routine")
        );
        assert_eq!(serialized["compatibility"]["version"], json!(1));
        assert_eq!(serialized["steps"][0]["kind"], json!("resolve"));
        assert_eq!(serialized["steps"][3]["kind"], json!("explore"));
        assert_eq!(serialized["steps"][5]["kind"], json!("compare"));
        assert_eq!(serialized["steps"][6]["kind"], json!("artifact-save"));
        assert_eq!(
            serialized["steps"][6]["artifact_id"],
            json!("artifact/focus-vs-neighbor")
        );

        let round_trip: WorkflowSpec =
            serde_json::from_value(serialized).expect("workflow spec should deserialize");
        assert_eq!(round_trip, workflow);
    }

    #[test]
    fn workflow_specs_default_legacy_compatibility_and_reject_future_versions() {
        let legacy_spec = json!({
            "workflow_id": "workflow/legacy",
            "title": "Legacy",
            "summary": null,
            "inputs": [],
            "steps": [{
                "step_id": "resolve-focus",
                "kind": "resolve",
                "target": {
                    "kind": "id",
                    "id": "focus-id"
                }
            }]
        });
        let legacy: WorkflowSpec =
            serde_json::from_value(legacy_spec).expect("legacy workflow spec should deserialize");
        assert_eq!(legacy.compatibility, WorkflowSpecCompatibility::default());
        assert_eq!(legacy.validation_error(), None);

        let future_spec = json!({
            "workflow_id": "workflow/future",
            "title": "Future",
            "summary": null,
            "compatibility": {
                "version": 2
            },
            "inputs": [],
            "steps": [{
                "step_id": "resolve-focus",
                "kind": "future-step",
                "future_field": true
            }]
        });
        let envelope: WorkflowSpecCompatibilityEnvelope =
            serde_json::from_value(future_spec.clone())
                .expect("future compatibility envelope should deserialize");
        assert_eq!(envelope.workflow_id.as_deref(), Some("workflow/future"));
        assert_eq!(
            envelope.compatibility.validation_error().as_deref(),
            Some("unsupported workflow spec compatibility version 2; supported version is 1")
        );
        serde_json::from_value::<WorkflowSpec>(future_spec)
            .expect_err("future workflow syntax should not deserialize as current spec");
    }

    #[test]
    fn workflow_execution_results_round_trip_with_typed_step_reports() {
        let workflow = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/round-trip".to_owned(),
                title: "Round Trip".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![
                WorkflowStepSpec {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Id {
                            id: "focus-id".to_owned(),
                        },
                    },
                },
                WorkflowStepSpec {
                    step_id: "explore-focus".to_owned(),
                    payload: WorkflowStepPayload::Explore {
                        focus: WorkflowExploreFocus::NodeKey {
                            node_key: "heading:focus.org:3".to_owned(),
                        },
                        lens: ExplorationLens::Structure,
                        limit: 10,
                        unique: false,
                    },
                },
                WorkflowStepSpec {
                    step_id: "compare-focus-neighbor".to_owned(),
                    payload: WorkflowStepPayload::Compare {
                        left: WorkflowStepRef {
                            step_id: "resolve-focus".to_owned(),
                        },
                        right: WorkflowStepRef {
                            step_id: "resolve-focus-other".to_owned(),
                        },
                        group: NoteComparisonGroup::All,
                        limit: 10,
                    },
                },
            ],
        };

        let executed_artifact = ExecutedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "artifact/focus".to_owned(),
                title: "Saved Focus".to_owned(),
                summary: None,
            },
            payload: ExecutedExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: "file:focus.org".to_owned(),
                    current_node_key: "heading:focus.org:3".to_owned(),
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: false,
                    frozen_context: true,
                }),
                root_note: Box::new(sample_node("file:focus.org", "Focus")),
                current_note: Box::new(sample_node("heading:focus.org:3", "Focus Heading")),
                result: Box::new(ExploreResult {
                    lens: ExplorationLens::Refs,
                    sections: Vec::new(),
                }),
            },
        };

        let result = WorkflowExecutionResult {
            workflow: WorkflowSummary::from(&workflow),
            steps: vec![
                WorkflowStepReport {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepReportPayload::Resolve {
                        node: Box::new(sample_node("heading:focus.org:3", "Focus")),
                    },
                },
                WorkflowStepReport {
                    step_id: "explore-focus".to_owned(),
                    payload: WorkflowStepReportPayload::Explore {
                        focus_node_key: "heading:focus.org:3".to_owned(),
                        result: Box::new(ExploreResult {
                            lens: ExplorationLens::Structure,
                            sections: Vec::new(),
                        }),
                    },
                },
                WorkflowStepReport {
                    step_id: "compare-focus-neighbor".to_owned(),
                    payload: WorkflowStepReportPayload::Compare {
                        left_node: Box::new(sample_node("heading:focus.org:3", "Focus")),
                        right_node: Box::new(sample_node("heading:neighbor.org:7", "Neighbor")),
                        result: Box::new(NoteComparisonResult {
                            left_note: sample_node("heading:focus.org:3", "Focus"),
                            right_note: sample_node("heading:neighbor.org:7", "Neighbor"),
                            sections: Vec::new(),
                        }),
                    },
                },
                WorkflowStepReport {
                    step_id: "run-artifact".to_owned(),
                    payload: WorkflowStepReportPayload::ArtifactRun {
                        artifact: Box::new(executed_artifact),
                    },
                },
                WorkflowStepReport {
                    step_id: "save-artifact".to_owned(),
                    payload: WorkflowStepReportPayload::ArtifactSave {
                        artifact: Box::new(ExplorationArtifactSummary {
                            metadata: ExplorationArtifactMetadata {
                                artifact_id: "artifact/focus".to_owned(),
                                title: "Saved Focus".to_owned(),
                                summary: None,
                            },
                            kind: ExplorationArtifactKind::LensView,
                        }),
                    },
                },
            ],
        };

        let serialized =
            serde_json::to_value(&result).expect("workflow execution result should serialize");
        assert_eq!(
            serialized["workflow"]["workflow_id"],
            json!("workflow/round-trip")
        );
        assert_eq!(serialized["steps"][0]["kind"], json!("resolve"));
        assert_eq!(serialized["steps"][1]["kind"], json!("explore"));
        assert_eq!(serialized["steps"][2]["kind"], json!("compare"));
        assert_eq!(serialized["steps"][3]["kind"], json!("artifact-run"));
        assert_eq!(serialized["steps"][4]["kind"], json!("artifact-save"));

        let round_trip: WorkflowExecutionResult = serde_json::from_value(serialized)
            .expect("workflow execution result should deserialize");
        assert_eq!(round_trip, result);

        let lines = result.report_lines();
        assert_eq!(lines.len(), 6);
        assert_eq!(
            serde_json::to_value(&lines[0]).expect("workflow report line should serialize"),
            json!({
                "kind": "workflow",
                "workflow": result.workflow
            })
        );
        assert_eq!(
            serde_json::to_value(&lines[1]).expect("workflow report line should serialize")["kind"],
            json!("step")
        );

        let round_trip_lines: Vec<WorkflowReportLine> = serde_json::from_value(
            serde_json::to_value(&lines).expect("workflow report lines should serialize"),
        )
        .expect("workflow report lines should deserialize");
        assert_eq!(round_trip_lines, lines);
    }

    #[test]
    fn corpus_audit_results_round_trip_with_typed_entries() {
        let result = CorpusAuditResult {
            audit: CorpusAuditKind::DanglingLinks,
            entries: vec![
                CorpusAuditEntry::DanglingLink {
                    record: Box::new(DanglingLinkAuditRecord {
                        source: sample_anchor("heading:source.org:3", "Source Heading"),
                        missing_explicit_id: "missing-id".to_owned(),
                        line: 12,
                        column: 7,
                        preview: "[[id:missing-id][Missing]]".to_owned(),
                    }),
                },
                CorpusAuditEntry::DuplicateTitle {
                    record: Box::new(DuplicateTitleAuditRecord {
                        title: "Shared Title".to_owned(),
                        notes: vec![
                            sample_node("file:left.org", "Shared Title"),
                            sample_node("file:right.org", "Shared Title"),
                        ],
                    }),
                },
                CorpusAuditEntry::OrphanNote {
                    record: Box::new(NoteConnectivityAuditRecord {
                        note: sample_node("file:orphan.org", "Orphan"),
                        reference_count: 0,
                        backlink_count: 0,
                        forward_link_count: 0,
                    }),
                },
                CorpusAuditEntry::WeaklyIntegratedNote {
                    record: Box::new(NoteConnectivityAuditRecord {
                        note: sample_node("file:weak.org", "Weak"),
                        reference_count: 2,
                        backlink_count: 0,
                        forward_link_count: 1,
                    }),
                },
            ],
        };

        let serialized = serde_json::to_value(&result).expect("audit result should serialize");
        assert_eq!(serialized["audit"], json!("dangling-links"));
        assert_eq!(serialized["entries"][0]["kind"], json!("dangling-link"));
        assert_eq!(serialized["entries"][1]["kind"], json!("duplicate-title"));
        assert_eq!(serialized["entries"][2]["kind"], json!("orphan-note"));
        assert_eq!(
            serialized["entries"][3]["kind"],
            json!("weakly-integrated-note")
        );

        let round_trip: CorpusAuditResult =
            serde_json::from_value(serialized).expect("audit result should deserialize");
        assert_eq!(round_trip, result);

        let lines = result.report_lines();
        assert_eq!(lines.len(), 5);
        assert_eq!(
            serde_json::to_value(&lines[0]).expect("audit report line should serialize"),
            json!({
                "kind": "audit",
                "audit": "dangling-links"
            })
        );
        assert_eq!(
            serde_json::to_value(&lines[1]).expect("audit report line should serialize")["kind"],
            json!("entry")
        );

        let round_trip_lines: Vec<CorpusAuditReportLine> = serde_json::from_value(
            serde_json::to_value(&lines).expect("audit report lines should serialize"),
        )
        .expect("audit report lines should deserialize");
        assert_eq!(round_trip_lines, lines);
    }

    #[test]
    fn report_profile_specs_round_trip_with_bounded_selections() {
        let profile = ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: "profile/review-diff-focus".to_owned(),
                title: "Review Diff Focus".to_owned(),
                summary: Some("Show open review details and selected diff buckets.".to_owned()),
            },
            subjects: vec![ReportProfileSubject::Review, ReportProfileSubject::Diff],
            mode: ReportProfileMode::Detail,
            status_filters: Some(vec![
                ReviewFindingStatus::Open,
                ReviewFindingStatus::Reviewed,
            ]),
            diff_buckets: Some(vec![
                ReviewRunDiffBucket::Added,
                ReviewRunDiffBucket::StatusChanged,
            ]),
            jsonl_line_kinds: Some(vec![
                ReportJsonlLineKind::Review,
                ReportJsonlLineKind::Finding,
                ReportJsonlLineKind::Diff,
                ReportJsonlLineKind::Added,
                ReportJsonlLineKind::StatusChanged,
            ]),
        };

        assert_eq!(profile.validation_error(), None);
        let serialized = serde_json::to_value(&profile).expect("profile should serialize");
        assert_eq!(serialized["profile_id"], json!("profile/review-diff-focus"));
        assert_eq!(serialized["subjects"], json!(["review", "diff"]));
        assert_eq!(serialized["mode"], json!("detail"));
        assert_eq!(serialized["status_filters"], json!(["open", "reviewed"]));
        assert_eq!(
            serialized["diff_buckets"],
            json!(["added", "status-changed"])
        );
        assert_eq!(
            serialized["jsonl_line_kinds"],
            json!(["review", "finding", "diff", "added", "status-changed"])
        );

        let round_trip: ReportProfileSpec =
            serde_json::from_value(serialized).expect("profile should deserialize");
        assert_eq!(round_trip, profile);

        let catalog = ReportProfileCatalog {
            profiles: vec![
                profile,
                ReportProfileSpec {
                    metadata: ReportProfileMetadata {
                        profile_id: "profile/workflow-summary".to_owned(),
                        title: "Workflow Summary".to_owned(),
                        summary: None,
                    },
                    subjects: vec![ReportProfileSubject::Workflow],
                    mode: ReportProfileMode::Summary,
                    status_filters: None,
                    diff_buckets: None,
                    jsonl_line_kinds: Some(vec![ReportJsonlLineKind::Workflow]),
                },
            ],
        };
        assert_eq!(catalog.validation_error(), None);
        let catalog_round_trip: ReportProfileCatalog = serde_json::from_value(
            serde_json::to_value(&catalog).expect("catalog should serialize"),
        )
        .expect("catalog should deserialize");
        assert_eq!(catalog_round_trip, catalog);
    }

    #[test]
    fn report_profile_specs_reject_malformed_and_contradictory_selections() {
        let valid = ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: "profile/review-open".to_owned(),
                title: "Review Open".to_owned(),
                summary: None,
            },
            subjects: vec![ReportProfileSubject::Review],
            mode: ReportProfileMode::Detail,
            status_filters: Some(vec![ReviewFindingStatus::Open]),
            diff_buckets: None,
            jsonl_line_kinds: Some(vec![ReportJsonlLineKind::Review]),
        };

        let mut padded_id = valid.clone();
        padded_id.metadata.profile_id = " profile/review-open".to_owned();
        assert_eq!(
            padded_id.validation_error().as_deref(),
            Some("profile_id must not have leading or trailing whitespace")
        );

        let mut empty_subjects = valid.clone();
        empty_subjects.subjects.clear();
        assert_eq!(
            empty_subjects.validation_error().as_deref(),
            Some("report profiles must select at least one subject")
        );

        let mut duplicate_subjects = valid.clone();
        duplicate_subjects.subjects =
            vec![ReportProfileSubject::Review, ReportProfileSubject::Review];
        assert_eq!(
            duplicate_subjects.validation_error().as_deref(),
            Some("report profile subject 1 is duplicate: review")
        );

        let mut empty_status_filters = valid.clone();
        empty_status_filters.status_filters = Some(Vec::new());
        assert_eq!(
            empty_status_filters.validation_error().as_deref(),
            Some("report profile status_filters must not be empty when present")
        );

        let mut duplicate_status_filters = valid.clone();
        duplicate_status_filters.status_filters =
            Some(vec![ReviewFindingStatus::Open, ReviewFindingStatus::Open]);
        assert_eq!(
            duplicate_status_filters.validation_error().as_deref(),
            Some("report profile status_filters entry 1 is duplicate: open")
        );

        let mut status_without_review_surface = valid.clone();
        status_without_review_surface.subjects = vec![ReportProfileSubject::Workflow];
        status_without_review_surface.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Workflow]);
        assert_eq!(
            status_without_review_surface.validation_error().as_deref(),
            Some("report profile status_filters require a review, routine, or diff subject")
        );

        let mut empty_diff_buckets = valid.clone();
        empty_diff_buckets.subjects = vec![ReportProfileSubject::Diff];
        empty_diff_buckets.status_filters = None;
        empty_diff_buckets.diff_buckets = Some(Vec::new());
        empty_diff_buckets.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Diff]);
        assert_eq!(
            empty_diff_buckets.validation_error().as_deref(),
            Some("report profile diff_buckets must not be empty when present")
        );

        let mut diff_without_diff_subject = valid.clone();
        diff_without_diff_subject.diff_buckets = Some(vec![ReviewRunDiffBucket::Added]);
        assert_eq!(
            diff_without_diff_subject.validation_error().as_deref(),
            Some("report profile diff_buckets require a diff subject")
        );

        let mut duplicate_diff_buckets = valid.clone();
        duplicate_diff_buckets.subjects = vec![ReportProfileSubject::Diff];
        duplicate_diff_buckets.status_filters = None;
        duplicate_diff_buckets.diff_buckets =
            Some(vec![ReviewRunDiffBucket::Added, ReviewRunDiffBucket::Added]);
        duplicate_diff_buckets.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Diff]);
        assert_eq!(
            duplicate_diff_buckets.validation_error().as_deref(),
            Some("report profile diff_buckets entry 1 is duplicate: added")
        );

        let mut empty_line_kinds = valid.clone();
        empty_line_kinds.jsonl_line_kinds = Some(Vec::new());
        assert_eq!(
            empty_line_kinds.validation_error().as_deref(),
            Some("report profile jsonl_line_kinds must not be empty when present")
        );

        let unsupported_line_kind: ReportProfileSpec = serde_json::from_value(json!({
            "profile_id": "profile/unsupported-line",
            "title": "Unsupported Line",
            "subjects": ["workflow"],
            "mode": "detail",
            "status_filters": null,
            "diff_buckets": null,
            "jsonl_line_kinds": ["workflow", "template-snippet"]
        }))
        .expect("unsupported line kind should deserialize for validation");
        assert_eq!(
            unsupported_line_kind.validation_error().as_deref(),
            Some("report profile jsonl_line_kinds entry 1 is unsupported: template-snippet")
        );

        let mut incompatible_line_kind = valid.clone();
        incompatible_line_kind.subjects = vec![ReportProfileSubject::Audit];
        incompatible_line_kind.status_filters = None;
        incompatible_line_kind.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Workflow]);
        assert_eq!(
            incompatible_line_kind.validation_error().as_deref(),
            Some(
                "report profile jsonl_line_kinds entry 0 is not supported by selected subjects: workflow"
            )
        );

        let mut summary_with_detail_line = valid.clone();
        summary_with_detail_line.mode = ReportProfileMode::Summary;
        summary_with_detail_line.jsonl_line_kinds = Some(vec![ReportJsonlLineKind::Finding]);
        assert_eq!(
            summary_with_detail_line.validation_error().as_deref(),
            Some("report profile summary mode cannot select detail JSONL line kind: finding")
        );

        let catalog = ReportProfileCatalog {
            profiles: vec![
                valid.clone(),
                ReportProfileSpec {
                    metadata: ReportProfileMetadata {
                        profile_id: valid.metadata.profile_id.clone(),
                        title: "Duplicate".to_owned(),
                        summary: None,
                    },
                    subjects: vec![ReportProfileSubject::Workflow],
                    mode: ReportProfileMode::Summary,
                    status_filters: None,
                    diff_buckets: None,
                    jsonl_line_kinds: Some(vec![ReportJsonlLineKind::Workflow]),
                },
            ],
        };
        assert_eq!(
            catalog.validation_error().as_deref(),
            Some("report profile 1 reuses duplicate profile_id profile/review-open")
        );
    }

    #[test]
    fn review_routine_specs_round_trip_over_audit_and_workflow_sources() {
        let audit_routine = ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: "routine/audit/duplicate-title-review".to_owned(),
                title: "Duplicate Title Review".to_owned(),
                summary: Some("Review title collisions and compare to the last run".to_owned()),
            },
            source: ReviewRoutineSource::Audit {
                audit: CorpusAuditKind::DuplicateTitles,
                limit: 100,
            },
            inputs: Vec::new(),
            save_review: ReviewRoutineSaveReviewPolicy {
                enabled: true,
                review_id: Some("review/routine/duplicate-title-review".to_owned()),
                title: Some("Duplicate Title Review".to_owned()),
                summary: Some("Generated by a declarative routine".to_owned()),
                overwrite: false,
            },
            compare: Some(ReviewRoutineComparePolicy {
                target: ReviewRoutineCompareTarget::LatestCompatibleReview,
                report_profile_id: Some("profile/diff-focus".to_owned()),
            }),
            report_profile_ids: vec![
                "profile/audit-detail".to_owned(),
                "profile/diff-focus".to_owned(),
            ],
        };
        assert_eq!(audit_routine.validation_error(), None);
        let serialized =
            serde_json::to_value(&audit_routine).expect("audit routine should serialize");
        assert_eq!(
            serialized["routine_id"],
            json!("routine/audit/duplicate-title-review")
        );
        assert_eq!(serialized["source"]["kind"], json!("audit"));
        assert_eq!(serialized["source"]["audit"], json!("duplicate-titles"));
        assert_eq!(
            serialized["compare"]["target"],
            json!("latest-compatible-review")
        );
        let round_trip: ReviewRoutineSpec =
            serde_json::from_value(serialized).expect("audit routine should deserialize");
        assert_eq!(round_trip, audit_routine);

        let workflow_routine = ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: "routine/workflow/periodic-review".to_owned(),
                title: "Periodic Workflow Review".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Workflow {
                workflow_id: BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID.to_owned(),
            },
            inputs: vec![WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Review focus".to_owned(),
                summary: Some("Note or anchor focus for the review".to_owned()),
                kind: WorkflowInputKind::FocusTarget,
            }],
            save_review: ReviewRoutineSaveReviewPolicy::default(),
            compare: None,
            report_profile_ids: vec!["profile/workflow-summary".to_owned()],
        };
        assert_eq!(workflow_routine.validation_error(), None);

        let catalog = ReviewRoutineCatalog {
            routines: vec![audit_routine, workflow_routine],
        };
        assert_eq!(catalog.validation_error(), None);
        let catalog_round_trip: ReviewRoutineCatalog = serde_json::from_value(
            serde_json::to_value(&catalog).expect("routine catalog should serialize"),
        )
        .expect("routine catalog should deserialize");
        assert_eq!(catalog_round_trip, catalog);
    }

    #[test]
    fn review_routine_specs_reject_invalid_references_and_policy_conflicts() {
        let valid = ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: "routine/workflow/context-review".to_owned(),
                title: "Context Review".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Workflow {
                workflow_id: BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned(),
            },
            inputs: vec![WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus".to_owned(),
                summary: None,
                kind: WorkflowInputKind::FocusTarget,
            }],
            save_review: ReviewRoutineSaveReviewPolicy::default(),
            compare: Some(ReviewRoutineComparePolicy {
                target: ReviewRoutineCompareTarget::LatestCompatibleReview,
                report_profile_id: Some("profile/diff-focus".to_owned()),
            }),
            report_profile_ids: vec!["profile/workflow-detail".to_owned()],
        };
        assert_eq!(valid.validation_error(), None);

        let mut padded_routine_id = valid.clone();
        padded_routine_id.metadata.routine_id = " routine/workflow/context-review".to_owned();
        assert_eq!(
            padded_routine_id.validation_error().as_deref(),
            Some("routine_id must not have leading or trailing whitespace")
        );

        let unsupported_source: ReviewRoutineSpec = serde_json::from_value(json!({
            "routine_id": "routine/future/source",
            "title": "Future Source",
            "source": {
                "kind": "script",
                "command": "external"
            }
        }))
        .expect("unsupported source kind should deserialize for validation");
        assert_eq!(
            unsupported_source.validation_error().as_deref(),
            Some("review routine source kind is unsupported")
        );

        let mut missing_workflow_reference = valid.clone();
        missing_workflow_reference.source = ReviewRoutineSource::Workflow {
            workflow_id: " ".to_owned(),
        };
        assert_eq!(
            missing_workflow_reference.validation_error().as_deref(),
            Some("workflow_id must not be empty")
        );

        let mut audit_with_inputs = valid.clone();
        audit_with_inputs.source = ReviewRoutineSource::Audit {
            audit: CorpusAuditKind::OrphanNotes,
            limit: 25,
        };
        assert_eq!(
            audit_with_inputs.validation_error().as_deref(),
            Some("audit review routines cannot declare workflow inputs")
        );

        let mut duplicate_inputs = valid.clone();
        duplicate_inputs.inputs.push(WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Duplicate Focus".to_owned(),
            summary: None,
            kind: WorkflowInputKind::FocusTarget,
        });
        assert_eq!(
            duplicate_inputs.validation_error().as_deref(),
            Some("review routine input 1 reuses duplicate input_id focus")
        );

        let mut disabled_save_with_metadata = valid.clone();
        disabled_save_with_metadata.save_review = ReviewRoutineSaveReviewPolicy {
            enabled: false,
            review_id: Some("review/routine/context".to_owned()),
            title: None,
            summary: None,
            overwrite: false,
        };
        disabled_save_with_metadata.compare = None;
        assert_eq!(
            disabled_save_with_metadata.validation_error().as_deref(),
            Some("disabled save_review policy cannot set review_id, title, summary, or overwrite")
        );

        let mut compare_without_save = valid.clone();
        compare_without_save.save_review = ReviewRoutineSaveReviewPolicy {
            enabled: false,
            review_id: None,
            title: None,
            summary: None,
            overwrite: false,
        };
        assert_eq!(
            compare_without_save.validation_error().as_deref(),
            Some("review routine compare policy requires save_review to be enabled")
        );

        let unsupported_compare: ReviewRoutineSpec = serde_json::from_value(json!({
            "routine_id": "routine/future/compare",
            "title": "Future Compare",
            "source": {
                "kind": "workflow",
                "workflow_id": BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID
            },
            "inputs": [],
            "compare": {
                "target": "scripted-baseline"
            }
        }))
        .expect("unsupported compare target should deserialize for validation");
        assert_eq!(
            unsupported_compare.validation_error().as_deref(),
            Some("review routine compare target is unsupported")
        );

        let mut padded_compare_profile = valid.clone();
        padded_compare_profile.compare = Some(ReviewRoutineComparePolicy {
            target: ReviewRoutineCompareTarget::LatestCompatibleReview,
            report_profile_id: Some(" profile/diff-focus".to_owned()),
        });
        assert_eq!(
            padded_compare_profile.validation_error().as_deref(),
            Some("report_profile_id must not have leading or trailing whitespace")
        );

        let mut padded_report_profile_ref = valid.clone();
        padded_report_profile_ref.report_profile_ids = vec![" profile/workflow-detail".to_owned()];
        assert_eq!(
            padded_report_profile_ref.validation_error().as_deref(),
            Some(
                "review routine report_profile_ids entry 0 is invalid: profile_id must not have leading or trailing whitespace"
            )
        );

        let mut duplicate_report_profile_ref = valid.clone();
        duplicate_report_profile_ref.report_profile_ids = vec![
            "profile/workflow-detail".to_owned(),
            "profile/workflow-detail".to_owned(),
        ];
        assert_eq!(
            duplicate_report_profile_ref.validation_error().as_deref(),
            Some("review routine report_profile_ids entry 1 is duplicate: profile/workflow-detail")
        );

        let catalog = ReviewRoutineCatalog {
            routines: vec![
                valid.clone(),
                ReviewRoutineSpec {
                    metadata: ReviewRoutineMetadata {
                        routine_id: valid.metadata.routine_id.clone(),
                        title: "Duplicate Routine".to_owned(),
                        summary: None,
                    },
                    source: ReviewRoutineSource::Audit {
                        audit: CorpusAuditKind::DanglingLinks,
                        limit: 200,
                    },
                    inputs: Vec::new(),
                    save_review: ReviewRoutineSaveReviewPolicy::default(),
                    compare: None,
                    report_profile_ids: Vec::new(),
                },
            ],
        };
        assert_eq!(
            catalog.validation_error().as_deref(),
            Some("review routine 1 reuses duplicate routine_id routine/workflow/context-review")
        );
    }

    #[test]
    fn workbench_pack_manifests_round_trip_with_bundled_assets() {
        let manifest = sample_workbench_pack_manifest();

        assert_eq!(manifest.validation_error(), None);
        assert!(manifest.validation_issues().is_empty());
        let summary = manifest.summary();
        assert_eq!(
            summary,
            WorkbenchPackSummary {
                metadata: manifest.metadata.clone(),
                compatibility: WorkbenchPackCompatibility::default(),
                workflow_count: 1,
                review_routine_count: 2,
                report_profile_count: 2,
                entrypoint_routine_ids: manifest.entrypoint_routine_ids.clone(),
            }
        );

        let serialized = serde_json::to_value(&manifest).expect("pack should serialize");
        assert_eq!(serialized["pack_id"], json!("pack/research-review"));
        assert_eq!(serialized["compatibility"]["version"], json!(1));
        assert_eq!(
            serialized["workflows"][0]["workflow_id"],
            json!("workflow/pack/context-review")
        );
        assert_eq!(
            serialized["review_routines"][0]["routine_id"],
            json!("routine/pack/context-review")
        );
        assert_eq!(
            serialized["report_profiles"][0]["profile_id"],
            json!("profile/routine-detail")
        );
        assert_eq!(
            serialized["entrypoint_routine_ids"],
            json!([
                "routine/pack/context-review",
                "routine/pack/duplicate-title-review"
            ])
        );

        let round_trip: WorkbenchPackManifest =
            serde_json::from_value(serialized).expect("pack should deserialize");
        assert_eq!(round_trip, manifest);

        let envelope: WorkbenchPackCompatibilityEnvelope = serde_json::from_value(json!({
            "pack_id": "pack/future",
            "compatibility": {
                "version": 2
            },
            "future_assets": [{
                "kind": "unknown"
            }]
        }))
        .expect("compatibility envelope should deserialize independently of future assets");
        assert_eq!(envelope.pack_id.as_deref(), Some("pack/future"));
        assert_eq!(
            envelope.compatibility.validation_error().as_deref(),
            Some("unsupported workbench pack compatibility version 2; supported version is 1")
        );
    }

    #[test]
    fn workbench_pack_rpc_contracts_round_trip() {
        let manifest = sample_workbench_pack_manifest();
        let summary = WorkbenchPackSummary::from(&manifest);

        let import_params: ImportWorkbenchPackParams = serde_json::from_value(json!({
            "pack": manifest.clone()
        }))
        .expect("import params should deserialize with default overwrite");
        assert!(!import_params.overwrite);
        assert_eq!(import_params.validation_error(), None);
        assert_eq!(import_params.pack, manifest);

        let explicit_overwrite: ImportWorkbenchPackParams = serde_json::from_value(json!({
            "pack": manifest.clone(),
            "overwrite": true
        }))
        .expect("explicit overwrite import params should deserialize");
        assert!(explicit_overwrite.overwrite);

        let validate_params = ValidateWorkbenchPackParams {
            pack: manifest.clone(),
        };
        let import_result = ImportWorkbenchPackResult {
            pack: summary.clone(),
        };
        let show_result = WorkbenchPackResult {
            pack: manifest.clone(),
        };
        let validate_result = ValidateWorkbenchPackResult {
            pack: Some(summary.clone()),
            valid: true,
            issues: Vec::new(),
        };
        let list_result = ListWorkbenchPacksResult {
            packs: vec![summary.clone()],
        };
        let delete_result = DeleteWorkbenchPackResult {
            pack_id: manifest.metadata.pack_id.clone(),
        };
        let id_params = WorkbenchPackIdParams {
            pack_id: manifest.metadata.pack_id.clone(),
        };
        assert_eq!(id_params.validation_error(), None);

        assert_eq!(
            serde_json::from_value::<ValidateWorkbenchPackParams>(
                serde_json::to_value(&validate_params).expect("validate params should serialize")
            )
            .expect("validate params should deserialize"),
            validate_params
        );
        assert_eq!(
            serde_json::from_value::<ImportWorkbenchPackResult>(
                serde_json::to_value(&import_result).expect("import result should serialize")
            )
            .expect("import result should deserialize"),
            import_result
        );
        assert_eq!(
            serde_json::from_value::<WorkbenchPackResult>(
                serde_json::to_value(&show_result).expect("show result should serialize")
            )
            .expect("show result should deserialize"),
            show_result
        );
        assert_eq!(
            serde_json::from_value::<ValidateWorkbenchPackResult>(
                serde_json::to_value(&validate_result).expect("validate result should serialize")
            )
            .expect("validate result should deserialize"),
            validate_result
        );
        assert_eq!(
            serde_json::from_value::<ListWorkbenchPacksResult>(
                serde_json::to_value(&list_result).expect("list result should serialize")
            )
            .expect("list result should deserialize"),
            list_result
        );
        assert_eq!(
            serde_json::from_value::<DeleteWorkbenchPackResult>(
                serde_json::to_value(&delete_result).expect("delete result should serialize")
            )
            .expect("delete result should deserialize"),
            delete_result
        );
        assert_eq!(
            serde_json::from_value::<WorkbenchPackIdParams>(
                serde_json::to_value(&id_params).expect("id params should serialize")
            )
            .expect("id params should deserialize"),
            id_params
        );
    }

    #[test]
    fn workbench_pack_manifests_report_malformed_assets_and_references() {
        let valid = sample_workbench_pack_manifest();
        assert_eq!(valid.validation_error(), None);

        let mut unsupported_version = valid.clone();
        unsupported_version.compatibility = WorkbenchPackCompatibility { version: 2 };
        let issues = unsupported_version.validation_issues();
        assert_eq!(issues[0].kind, WorkbenchPackIssueKind::UnsupportedVersion);
        assert_eq!(
            issues[0].message,
            "unsupported workbench pack compatibility version 2; supported version is 1"
        );

        let mut malformed_metadata = valid.clone();
        malformed_metadata.metadata.pack_id = " pack/research-review".to_owned();
        let issues = malformed_metadata.validation_issues();
        assert_eq!(issues[0].kind, WorkbenchPackIssueKind::InvalidMetadata);
        assert_eq!(
            issues[0].message,
            "pack_id must not have leading or trailing whitespace"
        );

        let empty_pack = WorkbenchPackManifest {
            metadata: WorkbenchPackMetadata {
                pack_id: "pack/empty".to_owned(),
                title: "Empty".to_owned(),
                summary: None,
            },
            compatibility: WorkbenchPackCompatibility::default(),
            workflows: Vec::new(),
            review_routines: Vec::new(),
            report_profiles: Vec::new(),
            entrypoint_routine_ids: Vec::new(),
        };
        let issues = empty_pack.validation_issues();
        assert_eq!(issues[0].kind, WorkbenchPackIssueKind::EmptyPack);

        let mut invalid_workflow = valid.clone();
        invalid_workflow.workflows[0].steps.clear();
        let issues = invalid_workflow.validation_issues();
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::InvalidWorkflow)
        );

        let mut invalid_routine = valid.clone();
        invalid_routine.review_routines[0]
            .inputs
            .push(WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Duplicate Focus".to_owned(),
                summary: None,
                kind: WorkflowInputKind::FocusTarget,
            });
        let issues = invalid_routine.validation_issues();
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::InvalidReviewRoutine)
        );

        let mut invalid_profile = valid.clone();
        invalid_profile.report_profiles[0].subjects.clear();
        let issues = invalid_profile.validation_issues();
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::InvalidReportProfile)
        );

        let mut duplicate_ids = valid.clone();
        duplicate_ids
            .workflows
            .push(duplicate_ids.workflows[0].clone());
        duplicate_ids
            .review_routines
            .push(duplicate_ids.review_routines[0].clone());
        duplicate_ids
            .report_profiles
            .push(duplicate_ids.report_profiles[0].clone());
        let issues = duplicate_ids.validation_issues();
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::DuplicateWorkflowId)
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::DuplicateReviewRoutineId)
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::DuplicateReportProfileId)
        );

        let mut built_in_collision = valid.clone();
        built_in_collision.workflows[0].metadata.workflow_id =
            BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned();
        built_in_collision.review_routines[0].source = ReviewRoutineSource::Workflow {
            workflow_id: BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned(),
        };
        built_in_collision.review_routines[0].inputs[0].kind = WorkflowInputKind::NoteTarget;
        let issues = built_in_collision.validation_issues();
        assert!(issues.iter().any(|issue| {
            issue.kind == WorkbenchPackIssueKind::DuplicateWorkflowId
                && issue.message
                    == format!(
                        "workbench pack workflow 0 collides with built-in workflow_id {BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID}"
                    )
        }));
        assert!(issues.iter().any(|issue| {
            issue.kind == WorkbenchPackIssueKind::InvalidReviewRoutineReference
                && issue
                    .message
                    .contains("but referenced workflow requires focus-target")
        }));

        let mut missing_workflow = valid.clone();
        missing_workflow.review_routines[0].source = ReviewRoutineSource::Workflow {
            workflow_id: "workflow/missing".to_owned(),
        };
        let issues = missing_workflow.validation_issues();
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::MissingWorkflowReference)
        );

        let mut mismatched_inputs = valid.clone();
        mismatched_inputs.review_routines[0].inputs[0].kind = WorkflowInputKind::NoteTarget;
        let issues = mismatched_inputs.validation_issues();
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::InvalidReviewRoutineReference)
        );

        let mut missing_profile = valid.clone();
        missing_profile.review_routines[0].report_profile_ids = vec!["profile/missing".to_owned()];
        let issues = missing_profile.validation_issues();
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::MissingReportProfileReference)
        );

        let mut missing_entrypoint = valid.clone();
        missing_entrypoint.entrypoint_routine_ids = vec![
            "routine/pack/context-review".to_owned(),
            "routine/pack/context-review".to_owned(),
            "routine/pack/missing".to_owned(),
        ];
        let issues = missing_entrypoint.validation_issues();
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::DuplicateReviewRoutineReference)
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == WorkbenchPackIssueKind::MissingReviewRoutineReference)
        );
    }

    #[test]
    fn corpus_audit_params_normalize_limit() {
        let params: CorpusAuditParams = serde_json::from_value(json!({
            "audit": "weakly-integrated-notes",
            "limit": 800
        }))
        .expect("audit params should deserialize");
        assert_eq!(params.audit, CorpusAuditKind::WeaklyIntegratedNotes);
        assert_eq!(params.normalized_limit(), 500);
        assert_eq!(
            serde_json::to_value(&params).expect("audit params should serialize"),
            json!({
                "audit": "weakly-integrated-notes",
                "limit": 800
            })
        );
    }

    #[test]
    fn review_runs_round_trip_with_audit_and_workflow_findings() {
        let audit_entry = CorpusAuditEntry::DanglingLink {
            record: Box::new(DanglingLinkAuditRecord {
                source: sample_anchor("heading:source.org:3", "Source Heading"),
                missing_explicit_id: "missing-id".to_owned(),
                line: 12,
                column: 7,
                preview: "[[id:missing-id][Missing]]".to_owned(),
            }),
        };
        let audit_review = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/audit/dangling-links/2026-05-05".to_owned(),
                title: "Dangling Link Review".to_owned(),
                summary: Some("Review missing id links".to_owned()),
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::DanglingLinks,
                limit: 200,
            },
            findings: vec![
                ReviewFinding {
                    finding_id: "audit/dangling-links/source/missing-id".to_owned(),
                    status: ReviewFindingStatus::Open,
                    payload: ReviewFindingPayload::Audit {
                        entry: Box::new(audit_entry.clone()),
                    },
                },
                ReviewFinding {
                    finding_id: "audit/dangling-links/source/other-missing-id".to_owned(),
                    status: ReviewFindingStatus::Dismissed,
                    payload: ReviewFindingPayload::Audit {
                        entry: Box::new(CorpusAuditEntry::DanglingLink {
                            record: Box::new(DanglingLinkAuditRecord {
                                source: sample_anchor("heading:source.org:3", "Source Heading"),
                                missing_explicit_id: "other-missing-id".to_owned(),
                                line: 18,
                                column: 3,
                                preview: "[[id:other-missing-id][Missing]]".to_owned(),
                            }),
                        }),
                    },
                },
            ],
        };

        assert_eq!(audit_review.validation_error(), None);
        assert_eq!(audit_review.kind(), super::ReviewRunKind::Audit);
        assert_eq!(
            audit_review.findings[0].kind(),
            super::ReviewFindingKind::Audit
        );

        let audit_summary = ReviewRunSummary::from(&audit_review);
        assert_eq!(audit_summary.finding_count, 2);
        assert_eq!(audit_summary.status_counts.open, 1);
        assert_eq!(audit_summary.status_counts.dismissed, 1);

        let serialized =
            serde_json::to_value(&audit_review).expect("audit review should serialize");
        assert_eq!(
            serialized["review_id"],
            json!("review/audit/dangling-links/2026-05-05")
        );
        assert_eq!(serialized["kind"], json!("audit"));
        assert_eq!(serialized["audit"], json!("dangling-links"));
        assert_eq!(serialized["limit"], json!(200));
        assert_eq!(serialized["findings"][0]["kind"], json!("audit"));
        assert_eq!(serialized["findings"][0]["status"], json!("open"));
        assert_eq!(
            serialized["findings"][0]["entry"]["kind"],
            json!("dangling-link")
        );

        let round_trip: ReviewRun =
            serde_json::from_value(serialized).expect("audit review should deserialize");
        assert_eq!(round_trip, audit_review);

        let workflow = WorkflowSummary {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/research/context".to_owned(),
                title: "Research Context".to_owned(),
                summary: Some("Collect review context".to_owned()),
            },
            step_count: 2,
        };
        let workflow_review = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/workflow/context/2026-05-05".to_owned(),
                title: "Workflow Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Workflow {
                workflow: workflow.clone(),
                inputs: vec![WorkflowInputAssignment {
                    input_id: "focus".to_owned(),
                    target: WorkflowResolveTarget::NodeKey {
                        node_key: "heading:focus.org:3".to_owned(),
                    },
                }],
                step_ids: vec!["resolve-focus".to_owned(), "explore-focus".to_owned()],
            },
            findings: vec![ReviewFinding {
                finding_id: "workflow-step/explore-focus".to_owned(),
                status: ReviewFindingStatus::Reviewed,
                payload: ReviewFindingPayload::WorkflowStep {
                    step: Box::new(WorkflowStepReport {
                        step_id: "explore-focus".to_owned(),
                        payload: WorkflowStepReportPayload::Explore {
                            focus_node_key: "heading:focus.org:3".to_owned(),
                            result: Box::new(ExploreResult {
                                lens: ExplorationLens::Unresolved,
                                sections: Vec::new(),
                            }),
                        },
                    }),
                },
            }],
        };

        assert_eq!(workflow_review.validation_error(), None);
        let workflow_json =
            serde_json::to_value(&workflow_review).expect("workflow review should serialize");
        assert_eq!(workflow_json["kind"], json!("workflow"));
        assert_eq!(
            workflow_json["workflow"]["workflow_id"],
            json!("workflow/research/context")
        );
        assert_eq!(
            workflow_json["step_ids"],
            json!(["resolve-focus", "explore-focus"])
        );
        assert_eq!(workflow_json["inputs"][0]["input_id"], json!("focus"));
        assert_eq!(
            workflow_json["inputs"][0]["node_key"],
            json!("heading:focus.org:3")
        );
        assert_eq!(workflow_json["findings"][0]["kind"], json!("workflow-step"));
        assert_eq!(workflow_json["findings"][0]["status"], json!("reviewed"));

        let audit_review_params = SaveCorpusAuditReviewParams {
            audit: CorpusAuditKind::DanglingLinks,
            limit: 50,
            review_id: Some("review/audit/dangling-links/custom".to_owned()),
            title: Some("Custom Dangling Review".to_owned()),
            summary: None,
            overwrite: false,
        };
        let workflow_review_params = SaveWorkflowReviewParams {
            workflow_id: "workflow/research/context".to_owned(),
            inputs: vec![WorkflowInputAssignment {
                input_id: "focus".to_owned(),
                target: WorkflowResolveTarget::NodeKey {
                    node_key: "heading:focus.org:3".to_owned(),
                },
            }],
            review_id: None,
            title: Some("Custom Workflow Review".to_owned()),
            summary: None,
            overwrite: false,
        };
        let save_params = SaveReviewRunParams {
            review: audit_review.clone(),
            overwrite: false,
        };
        let mark_params = MarkReviewFindingParams {
            review_id: "review/workflow/context/2026-05-05".to_owned(),
            finding_id: "workflow-step/explore-focus".to_owned(),
            status: ReviewFindingStatus::Accepted,
        };
        let preview_params = ReviewFindingRemediationPreviewParams {
            review_id: "review/audit/dangling-links/2026-05-05".to_owned(),
            finding_id: "audit/dangling-links/source/missing-id".to_owned(),
        };
        let diff_params = ReviewRunDiffParams {
            base_review_id: "review/audit/dangling-links/2026-05-04".to_owned(),
            target_review_id: "review/audit/dangling-links/2026-05-05".to_owned(),
        };
        let save_result = SaveReviewRunResult {
            review: ReviewRunSummary::from(&audit_review),
        };
        let review_result = ReviewRunResult {
            review: workflow_review.clone(),
        };
        let list_result = super::ListReviewRunsResult {
            reviews: vec![
                ReviewRunSummary::from(&audit_review),
                ReviewRunSummary::from(&workflow_review),
            ],
        };
        let delete_result = super::DeleteReviewRunResult {
            review_id: "review/workflow/context/2026-05-05".to_owned(),
        };
        let mark_result = super::MarkReviewFindingResult {
            transition: ReviewFindingStatusTransition {
                review_id: "review/workflow/context/2026-05-05".to_owned(),
                finding_id: "workflow-step/explore-focus".to_owned(),
                from_status: ReviewFindingStatus::Open,
                to_status: ReviewFindingStatus::Reviewed,
            },
        };
        let audit_review_result = SaveCorpusAuditReviewResult {
            result: CorpusAuditResult {
                audit: CorpusAuditKind::DanglingLinks,
                entries: vec![audit_entry],
            },
            review: ReviewRunSummary::from(&audit_review),
        };
        let workflow_review_result = SaveWorkflowReviewResult {
            result: WorkflowExecutionResult {
                workflow,
                steps: vec![match &workflow_review.findings[0].payload {
                    ReviewFindingPayload::WorkflowStep { step } => step.as_ref().clone(),
                    _ => panic!("expected workflow-step finding"),
                }],
            },
            review: ReviewRunSummary::from(&workflow_review),
        };
        let diff_result = ReviewRunDiffResult {
            diff: ReviewRunDiff::between(&audit_review, &audit_review)
                .expect("same audit review should diff"),
        };
        let preview_result = ReviewFindingRemediationPreviewResult {
            preview: ReviewFindingRemediationPreview::from_review_finding(
                "review/audit/dangling-links/2026-05-05",
                &audit_review.findings[0],
            )
            .expect("dangling-link finding should have a preview"),
        };

        assert_eq!(
            serde_json::from_value::<SaveReviewRunResult>(
                serde_json::to_value(&save_result).expect("save result should serialize"),
            )
            .expect("save result should deserialize"),
            save_result
        );
        assert_eq!(
            serde_json::from_value::<ReviewRunResult>(
                serde_json::to_value(&review_result).expect("review result should serialize"),
            )
            .expect("review result should deserialize"),
            review_result
        );
        assert_eq!(
            serde_json::from_value::<super::ListReviewRunsResult>(
                serde_json::to_value(&list_result).expect("list result should serialize"),
            )
            .expect("list result should deserialize"),
            list_result
        );
        assert_eq!(
            serde_json::from_value::<super::DeleteReviewRunResult>(
                serde_json::to_value(&delete_result).expect("delete result should serialize"),
            )
            .expect("delete result should deserialize"),
            delete_result
        );
        assert_eq!(
            serde_json::from_value::<super::MarkReviewFindingResult>(
                serde_json::to_value(&mark_result).expect("mark result should serialize"),
            )
            .expect("mark result should deserialize"),
            mark_result
        );
        assert_eq!(
            serde_json::from_value::<SaveReviewRunParams>(
                serde_json::to_value(&save_params).expect("save params should serialize"),
            )
            .expect("save params should deserialize"),
            save_params
        );
        assert_eq!(
            serde_json::from_value::<MarkReviewFindingParams>(
                serde_json::to_value(&mark_params).expect("mark params should serialize"),
            )
            .expect("mark params should deserialize"),
            mark_params
        );
        assert_eq!(
            serde_json::from_value::<ReviewRunDiffParams>(
                serde_json::to_value(&diff_params).expect("diff params should serialize"),
            )
            .expect("diff params should deserialize"),
            diff_params
        );
        assert_eq!(
            serde_json::from_value::<ReviewFindingRemediationPreviewParams>(
                serde_json::to_value(&preview_params).expect("preview params should serialize"),
            )
            .expect("preview params should deserialize"),
            preview_params
        );
        assert_eq!(
            serde_json::from_value::<SaveCorpusAuditReviewParams>(
                serde_json::to_value(&audit_review_params)
                    .expect("audit review params should serialize"),
            )
            .expect("audit review params should deserialize"),
            audit_review_params
        );
        assert_eq!(
            serde_json::from_value::<SaveWorkflowReviewParams>(
                serde_json::to_value(&workflow_review_params)
                    .expect("workflow review params should serialize"),
            )
            .expect("workflow review params should deserialize"),
            workflow_review_params
        );
        assert_eq!(
            serde_json::from_value::<SaveCorpusAuditReviewResult>(
                serde_json::to_value(&audit_review_result)
                    .expect("audit review result should serialize"),
            )
            .expect("audit review result should deserialize"),
            audit_review_result
        );
        assert_eq!(
            serde_json::from_value::<SaveWorkflowReviewResult>(
                serde_json::to_value(&workflow_review_result)
                    .expect("workflow review result should serialize"),
            )
            .expect("workflow review result should deserialize"),
            workflow_review_result
        );
        assert_eq!(
            serde_json::from_value::<ReviewRunDiffResult>(
                serde_json::to_value(&diff_result).expect("diff result should serialize"),
            )
            .expect("diff result should deserialize"),
            diff_result
        );
        assert_eq!(
            serde_json::from_value::<ReviewFindingRemediationPreviewResult>(
                serde_json::to_value(&preview_result).expect("preview result should serialize"),
            )
            .expect("preview result should deserialize"),
            preview_result
        );

        let default_save_params: SaveReviewRunParams = serde_json::from_value(json!({
            "review": audit_review
        }))
        .expect("save params should default overwrite");
        assert!(default_save_params.overwrite);
    }

    #[test]
    fn review_finding_remediation_previews_cover_supported_audit_findings() {
        let dangling_finding = ReviewFinding {
            finding_id: "audit/dangling-links/source/missing-id".to_owned(),
            status: ReviewFindingStatus::Open,
            payload: ReviewFindingPayload::Audit {
                entry: Box::new(CorpusAuditEntry::DanglingLink {
                    record: Box::new(DanglingLinkAuditRecord {
                        source: sample_anchor("file:source.org", "Source"),
                        missing_explicit_id: "missing-id".to_owned(),
                        line: 12,
                        column: 7,
                        preview: "[[id:missing-id][Missing]]".to_owned(),
                    }),
                }),
            },
        };
        let dangling_preview = ReviewFindingRemediationPreview::from_review_finding(
            "review/audit/dangling-links",
            &dangling_finding,
        )
        .expect("dangling link should be previewable");
        assert_eq!(dangling_preview.review_id, "review/audit/dangling-links");
        assert_eq!(
            dangling_preview.finding_id,
            "audit/dangling-links/source/missing-id"
        );
        assert_eq!(dangling_preview.status, ReviewFindingStatus::Open);
        match dangling_preview.payload {
            super::AuditRemediationPreviewPayload::DanglingLink {
                source,
                missing_explicit_id,
                file_path,
                line,
                column,
                preview,
                suggestion,
                confidence,
                reason,
            } => {
                assert_eq!(source.node_key, "file:source.org");
                assert_eq!(missing_explicit_id, "missing-id");
                assert_eq!(file_path, "sample.org");
                assert_eq!(line, 12);
                assert_eq!(column, 7);
                assert_eq!(preview, "[[id:missing-id][Missing]]");
                assert!(suggestion.contains("id:missing-id"));
                assert_eq!(confidence, super::AuditRemediationConfidence::Medium);
                assert!(reason.contains("missing-id"));
            }
            other => panic!("expected dangling-link preview, got {other:?}"),
        }

        let duplicate_finding = ReviewFinding {
            finding_id: "audit/duplicate-titles/shared-title".to_owned(),
            status: ReviewFindingStatus::Reviewed,
            payload: ReviewFindingPayload::Audit {
                entry: Box::new(CorpusAuditEntry::DuplicateTitle {
                    record: Box::new(DuplicateTitleAuditRecord {
                        title: "Shared Title".to_owned(),
                        notes: vec![
                            sample_node("file:a.org", "Shared Title"),
                            sample_node("file:b.org", "Shared Title"),
                        ],
                    }),
                }),
            },
        };
        let duplicate_preview = ReviewFindingRemediationPreview::from_review_finding(
            "review/audit/duplicate-titles",
            &duplicate_finding,
        )
        .expect("duplicate title should be previewable");
        match duplicate_preview.payload {
            super::AuditRemediationPreviewPayload::DuplicateTitle {
                title,
                notes,
                suggestion,
                confidence,
                reason,
            } => {
                assert_eq!(title, "Shared Title");
                assert_eq!(notes.len(), 2);
                assert!(suggestion.contains("Disambiguate"));
                assert_eq!(confidence, super::AuditRemediationConfidence::High);
                assert!(reason.contains("2 notes"));
            }
            other => panic!("expected duplicate-title preview, got {other:?}"),
        }

        let unsupported = ReviewFinding {
            finding_id: "audit/orphan-notes/source".to_owned(),
            status: ReviewFindingStatus::Open,
            payload: ReviewFindingPayload::Audit {
                entry: Box::new(CorpusAuditEntry::OrphanNote {
                    record: Box::new(NoteConnectivityAuditRecord {
                        note: sample_node("file:orphan.org", "Orphan"),
                        reference_count: 0,
                        backlink_count: 0,
                        forward_link_count: 0,
                    }),
                }),
            },
        };
        assert_eq!(
            ReviewFindingRemediationPreview::from_review_finding(
                "review/audit/orphan-notes",
                &unsupported,
            )
            .expect_err("orphan finding should not be previewable"),
            "review finding has no remediation preview for orphan-note evidence"
        );
    }

    #[test]
    fn review_run_diffs_classify_findings_deterministically() {
        let base = sample_dangling_review(
            "review/audit/dangling-links/base",
            vec![
                sample_dangling_finding(
                    "audit/dangling-links/added-order/status-changed",
                    "status-changed",
                    ReviewFindingStatus::Open,
                ),
                sample_dangling_finding(
                    "audit/dangling-links/added-order/removed",
                    "removed",
                    ReviewFindingStatus::Dismissed,
                ),
                sample_dangling_finding(
                    "audit/dangling-links/added-order/unchanged",
                    "unchanged",
                    ReviewFindingStatus::Reviewed,
                ),
            ],
        );
        let target = sample_dangling_review(
            "review/audit/dangling-links/target",
            vec![
                sample_dangling_finding(
                    "audit/dangling-links/added-order/unchanged",
                    "unchanged",
                    ReviewFindingStatus::Reviewed,
                ),
                sample_dangling_finding(
                    "audit/dangling-links/added-order/added",
                    "added",
                    ReviewFindingStatus::Open,
                ),
                sample_dangling_finding(
                    "audit/dangling-links/added-order/status-changed",
                    "status-changed",
                    ReviewFindingStatus::Accepted,
                ),
            ],
        );

        let diff = ReviewRunDiff::between(&base, &target).expect("compatible reviews should diff");

        assert_eq!(diff.base_review.metadata.review_id, base.metadata.review_id);
        assert_eq!(
            diff.target_review.metadata.review_id,
            target.metadata.review_id
        );
        assert_eq!(
            diff.added
                .iter()
                .map(|finding| finding.finding_id.as_str())
                .collect::<Vec<_>>(),
            vec!["audit/dangling-links/added-order/added"]
        );
        assert_eq!(
            diff.removed
                .iter()
                .map(|finding| finding.finding_id.as_str())
                .collect::<Vec<_>>(),
            vec!["audit/dangling-links/added-order/removed"]
        );
        assert_eq!(
            diff.unchanged
                .iter()
                .map(|finding| finding.finding_id.as_str())
                .collect::<Vec<_>>(),
            vec!["audit/dangling-links/added-order/unchanged"]
        );
        assert!(diff.content_changed.is_empty());
        assert_eq!(diff.status_changed.len(), 1);
        assert_eq!(
            diff.status_changed[0].finding_id,
            "audit/dangling-links/added-order/status-changed"
        );
        assert_eq!(
            diff.status_changed[0].from_status,
            ReviewFindingStatus::Open
        );
        assert_eq!(
            diff.status_changed[0].to_status,
            ReviewFindingStatus::Accepted
        );
    }

    #[test]
    fn review_run_diffs_separate_content_changes_from_unchanged_findings() {
        let workflow = WorkflowSummary {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/research/context".to_owned(),
                title: "Research Context".to_owned(),
                summary: None,
            },
            step_count: 1,
        };
        let base = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/workflow/context/base".to_owned(),
                title: "Workflow Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Workflow {
                workflow: workflow.clone(),
                inputs: Vec::new(),
                step_ids: vec!["resolve-focus".to_owned()],
            },
            findings: vec![ReviewFinding {
                finding_id: "workflow-step/resolve-focus".to_owned(),
                status: ReviewFindingStatus::Open,
                payload: ReviewFindingPayload::WorkflowStep {
                    step: Box::new(WorkflowStepReport {
                        step_id: "resolve-focus".to_owned(),
                        payload: WorkflowStepReportPayload::Resolve {
                            node: Box::new(sample_node("heading:focus.org:3", "Old Focus")),
                        },
                    }),
                },
            }],
        };
        let target = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/workflow/context/target".to_owned(),
                title: "Workflow Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Workflow {
                workflow,
                inputs: Vec::new(),
                step_ids: vec!["resolve-focus".to_owned()],
            },
            findings: vec![ReviewFinding {
                finding_id: "workflow-step/resolve-focus".to_owned(),
                status: ReviewFindingStatus::Open,
                payload: ReviewFindingPayload::WorkflowStep {
                    step: Box::new(WorkflowStepReport {
                        step_id: "resolve-focus".to_owned(),
                        payload: WorkflowStepReportPayload::Resolve {
                            node: Box::new(sample_node("heading:focus.org:3", "New Focus")),
                        },
                    }),
                },
            }],
        };

        let diff = ReviewRunDiff::between(&base, &target).expect("compatible reviews should diff");

        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.unchanged.is_empty());
        assert!(diff.status_changed.is_empty());
        assert_eq!(diff.content_changed.len(), 1);
        assert_eq!(
            diff.content_changed[0].finding_id,
            "workflow-step/resolve-focus"
        );
        match &diff.content_changed[0].base.payload {
            ReviewFindingPayload::WorkflowStep { step } => match &step.payload {
                WorkflowStepReportPayload::Resolve { node } => assert_eq!(node.title, "Old Focus"),
                other => panic!("expected resolve payload, got {:?}", other.kind()),
            },
            other => panic!("expected workflow-step payload, got {:?}", other.kind()),
        }
        match &diff.content_changed[0].target.payload {
            ReviewFindingPayload::WorkflowStep { step } => match &step.payload {
                WorkflowStepReportPayload::Resolve { node } => assert_eq!(node.title, "New Focus"),
                other => panic!("expected resolve payload, got {:?}", other.kind()),
            },
            other => panic!("expected workflow-step payload, got {:?}", other.kind()),
        }
    }

    #[test]
    fn review_run_diffs_reject_incompatible_review_sources() {
        let audit_review = sample_dangling_review(
            "review/audit/dangling-links",
            vec![sample_dangling_finding(
                "audit/dangling-links/source/missing-id",
                "missing-id",
                ReviewFindingStatus::Open,
            )],
        );
        let different_audit = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/audit/orphan-notes".to_owned(),
                title: "Orphan Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::OrphanNotes,
                limit: 200,
            },
            findings: Vec::new(),
        };
        let audit_error = ReviewRunDiff::between(&audit_review, &different_audit)
            .expect_err("different audit kinds should be incompatible");
        assert!(audit_error.contains("different audit kinds"));

        let workflow = WorkflowSummary {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/research/context".to_owned(),
                title: "Research Context".to_owned(),
                summary: None,
            },
            step_count: 1,
        };
        let workflow_review = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/workflow/context".to_owned(),
                title: "Workflow Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Workflow {
                workflow: workflow.clone(),
                inputs: Vec::new(),
                step_ids: vec!["resolve-focus".to_owned()],
            },
            findings: Vec::new(),
        };
        let changed_workflow_review = ReviewRun {
            payload: ReviewRunPayload::Workflow {
                workflow,
                inputs: vec![WorkflowInputAssignment {
                    input_id: "focus".to_owned(),
                    target: WorkflowResolveTarget::NodeKey {
                        node_key: "heading:focus.org:3".to_owned(),
                    },
                }],
                step_ids: vec!["resolve-focus".to_owned()],
            },
            ..workflow_review.clone()
        };
        let workflow_error = ReviewRunDiff::between(&workflow_review, &changed_workflow_review)
            .expect_err("different workflow inputs should be incompatible");
        assert!(workflow_error.contains("different inputs"));

        let cross_kind_error = ReviewRunDiff::between(&audit_review, &workflow_review)
            .expect_err("cross-kind reviews should be incompatible");
        assert_eq!(
            cross_kind_error,
            "cannot diff review runs with different kinds"
        );
    }

    #[test]
    fn review_runs_reject_malformed_records_and_invalid_status_transitions() {
        let valid_finding = ReviewFinding {
            finding_id: "audit/dangling-links/source/missing-id".to_owned(),
            status: ReviewFindingStatus::Open,
            payload: ReviewFindingPayload::Audit {
                entry: Box::new(CorpusAuditEntry::DanglingLink {
                    record: Box::new(DanglingLinkAuditRecord {
                        source: sample_anchor("heading:source.org:3", "Source Heading"),
                        missing_explicit_id: "missing-id".to_owned(),
                        line: 12,
                        column: 7,
                        preview: "[[id:missing-id][Missing]]".to_owned(),
                    }),
                }),
            },
        };

        let blank_metadata = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: " ".to_owned(),
                title: "Dangling Link Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::DanglingLinks,
                limit: 200,
            },
            findings: vec![valid_finding.clone()],
        };
        assert_eq!(
            blank_metadata.validation_error().as_deref(),
            Some("review_id must not be empty")
        );

        let padded_id = ReviewRunIdParams {
            review_id: " review/audit ".to_owned(),
        };
        assert_eq!(
            padded_id.validation_error().as_deref(),
            Some("review_id must not have leading or trailing whitespace")
        );

        let duplicate_findings = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/audit/dangling-links".to_owned(),
                title: "Dangling Link Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::DanglingLinks,
                limit: 200,
            },
            findings: vec![valid_finding.clone(), valid_finding.clone()],
        };
        assert_eq!(
            duplicate_findings.validation_error().as_deref(),
            Some(
                "review finding 1 reuses duplicate finding_id audit/dangling-links/source/missing-id"
            )
        );

        let padded_finding = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/audit/dangling-links".to_owned(),
                title: "Dangling Link Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::DanglingLinks,
                limit: 200,
            },
            findings: vec![ReviewFinding {
                finding_id: " audit/finding ".to_owned(),
                ..valid_finding.clone()
            }],
        };
        assert_eq!(
            padded_finding.validation_error().as_deref(),
            Some(
                "review finding 0 is invalid: finding_id must not have leading or trailing whitespace"
            )
        );

        let wrong_audit_kind = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/audit/orphans".to_owned(),
                title: "Orphan Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::OrphanNotes,
                limit: 200,
            },
            findings: vec![valid_finding.clone()],
        };
        assert_eq!(
            wrong_audit_kind.validation_error().as_deref(),
            Some("review finding 0 is invalid: audit review findings must match review audit kind")
        );

        let workflow_step_in_audit = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/audit/dangling-links".to_owned(),
                title: "Dangling Link Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::DanglingLinks,
                limit: 200,
            },
            findings: vec![ReviewFinding {
                finding_id: "workflow-step/explore-focus".to_owned(),
                status: ReviewFindingStatus::Reviewed,
                payload: ReviewFindingPayload::WorkflowStep {
                    step: Box::new(WorkflowStepReport {
                        step_id: "explore-focus".to_owned(),
                        payload: WorkflowStepReportPayload::Explore {
                            focus_node_key: "heading:focus.org:3".to_owned(),
                            result: Box::new(ExploreResult {
                                lens: ExplorationLens::Unresolved,
                                sections: Vec::new(),
                            }),
                        },
                    }),
                },
            }],
        };
        assert_eq!(
            workflow_step_in_audit.validation_error().as_deref(),
            Some(
                "review finding 0 is invalid: audit review runs cannot contain workflow-step findings"
            )
        );

        let malformed_audit_entry = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/audit/dangling-links".to_owned(),
                title: "Dangling Link Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::DanglingLinks,
                limit: 200,
            },
            findings: vec![ReviewFinding {
                payload: ReviewFindingPayload::Audit {
                    entry: Box::new(CorpusAuditEntry::DanglingLink {
                        record: Box::new(DanglingLinkAuditRecord {
                            source: sample_anchor("", "Source Heading"),
                            missing_explicit_id: "missing-id".to_owned(),
                            line: 12,
                            column: 7,
                            preview: "[[id:missing-id][Missing]]".to_owned(),
                        }),
                    }),
                },
                ..valid_finding
            }],
        };
        assert_eq!(
            malformed_audit_entry.validation_error().as_deref(),
            Some("review finding 0 is invalid: source.node_key must not be empty")
        );

        let workflow = WorkflowSummary {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/research/context".to_owned(),
                title: "Research Context".to_owned(),
                summary: None,
            },
            step_count: 2,
        };
        let workflow_step = WorkflowStepReport {
            step_id: "explore-focus".to_owned(),
            payload: WorkflowStepReportPayload::Explore {
                focus_node_key: "heading:focus.org:3".to_owned(),
                result: Box::new(ExploreResult {
                    lens: ExplorationLens::Unresolved,
                    sections: Vec::new(),
                }),
            },
        };

        let mismatched_workflow_source = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/workflow/context".to_owned(),
                title: "Workflow Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Workflow {
                workflow: workflow.clone(),
                inputs: Vec::new(),
                step_ids: vec!["explore-focus".to_owned()],
            },
            findings: Vec::new(),
        };
        assert_eq!(
            mismatched_workflow_source.validation_error().as_deref(),
            Some("workflow review source step_ids must match workflow step_count")
        );

        let duplicate_source_step_ids = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/workflow/context".to_owned(),
                title: "Workflow Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Workflow {
                workflow: workflow.clone(),
                inputs: Vec::new(),
                step_ids: vec!["explore-focus".to_owned(), "explore-focus".to_owned()],
            },
            findings: Vec::new(),
        };
        assert_eq!(
            duplicate_source_step_ids.validation_error().as_deref(),
            Some("workflow review source step_id 1 reuses duplicate step_id explore-focus")
        );

        let unknown_workflow_step = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/workflow/context".to_owned(),
                title: "Workflow Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Workflow {
                workflow: workflow.clone(),
                inputs: Vec::new(),
                step_ids: vec!["resolve-focus".to_owned(), "explore-focus".to_owned()],
            },
            findings: vec![ReviewFinding {
                finding_id: "workflow-step/compare-focus".to_owned(),
                status: ReviewFindingStatus::Open,
                payload: ReviewFindingPayload::WorkflowStep {
                    step: Box::new(WorkflowStepReport {
                        step_id: "compare-focus".to_owned(),
                        ..workflow_step.clone()
                    }),
                },
            }],
        };
        assert_eq!(
            unknown_workflow_step.validation_error().as_deref(),
            Some(
                "review finding 0 is invalid: workflow-step findings must reference a source workflow step"
            )
        );

        let duplicate_workflow_step_finding = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/workflow/context".to_owned(),
                title: "Workflow Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Workflow {
                workflow,
                inputs: Vec::new(),
                step_ids: vec!["resolve-focus".to_owned(), "explore-focus".to_owned()],
            },
            findings: vec![
                ReviewFinding {
                    finding_id: "workflow-step/explore-focus".to_owned(),
                    status: ReviewFindingStatus::Open,
                    payload: ReviewFindingPayload::WorkflowStep {
                        step: Box::new(workflow_step.clone()),
                    },
                },
                ReviewFinding {
                    finding_id: "workflow-step/explore-focus-copy".to_owned(),
                    status: ReviewFindingStatus::Reviewed,
                    payload: ReviewFindingPayload::WorkflowStep {
                        step: Box::new(workflow_step),
                    },
                },
            ],
        };
        assert_eq!(
            duplicate_workflow_step_finding
                .validation_error()
                .as_deref(),
            Some("review finding 1 reuses duplicate workflow step_id explore-focus")
        );

        let no_op_transition = ReviewFindingStatusTransition {
            review_id: "review/audit/dangling-links".to_owned(),
            finding_id: "audit/dangling-links/source/missing-id".to_owned(),
            from_status: ReviewFindingStatus::Open,
            to_status: ReviewFindingStatus::Open,
        };
        assert_eq!(
            no_op_transition.validation_error().as_deref(),
            Some("review finding status transition must change status")
        );
    }

    #[test]
    fn workflow_specs_reject_malformed_metadata_and_step_references() {
        let blank_metadata = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: " ".to_owned(),
                title: "Workflow".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Id {
                        id: "focus-id".to_owned(),
                    },
                },
            }],
        };
        assert_eq!(
            blank_metadata.validation_error().as_deref(),
            Some("workflow_id must not be empty")
        );

        let empty_steps = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/empty".to_owned(),
                title: "Empty".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: Vec::new(),
        };
        assert_eq!(
            empty_steps.validation_error().as_deref(),
            Some("workflows must contain at least one step")
        );

        let duplicate_step_ids = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/duplicate".to_owned(),
                title: "Duplicate".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![
                WorkflowStepSpec {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Id {
                            id: "focus-id".to_owned(),
                        },
                    },
                },
                WorkflowStepSpec {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Title {
                            title: "Other".to_owned(),
                        },
                    },
                },
            ],
        };
        assert_eq!(
            duplicate_step_ids.validation_error().as_deref(),
            Some("workflow step 1 reuses duplicate step_id resolve-focus")
        );

        let missing_reference = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/missing-ref".to_owned(),
                title: "Missing Ref".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![WorkflowStepSpec {
                step_id: "explore-focus".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::ResolvedStep {
                        step_id: "resolve-focus".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: false,
                },
            }],
        };
        assert_eq!(
            missing_reference.validation_error().as_deref(),
            Some("workflow step 0 is invalid: focus must reference an earlier resolve step")
        );

        let wrong_reference_kind = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/wrong-ref".to_owned(),
                title: "Wrong Ref".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![
                WorkflowStepSpec {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Id {
                            id: "focus-id".to_owned(),
                        },
                    },
                },
                WorkflowStepSpec {
                    step_id: "save-focus".to_owned(),
                    payload: WorkflowStepPayload::ArtifactSave {
                        source: WorkflowArtifactSaveSource::CompareStep {
                            step_id: "resolve-focus".to_owned(),
                        },
                        metadata: ExplorationArtifactMetadata {
                            artifact_id: "artifact/focus".to_owned(),
                            title: "Focus".to_owned(),
                            summary: None,
                        },
                        overwrite: true,
                    },
                },
            ],
        };
        assert_eq!(
            wrong_reference_kind.validation_error().as_deref(),
            Some("workflow step 1 is invalid: source must reference a compare step, not resolve")
        );

        let same_compare_refs = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/same-compare".to_owned(),
                title: "Same Compare".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![
                WorkflowStepSpec {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Id {
                            id: "focus-id".to_owned(),
                        },
                    },
                },
                WorkflowStepSpec {
                    step_id: "compare-focus".to_owned(),
                    payload: WorkflowStepPayload::Compare {
                        left: WorkflowStepRef {
                            step_id: "resolve-focus".to_owned(),
                        },
                        right: WorkflowStepRef {
                            step_id: "resolve-focus".to_owned(),
                        },
                        group: NoteComparisonGroup::All,
                        limit: 25,
                    },
                },
            ],
        };
        assert_eq!(
            same_compare_refs.validation_error().as_deref(),
            Some(
                "workflow step 1 is invalid: compare left and right must reference distinct resolve steps"
            )
        );

        let invalid_explore_unique = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/unique-refs".to_owned(),
                title: "Unique Refs".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![WorkflowStepSpec {
                step_id: "explore-focus".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::NodeKey {
                        node_key: "heading:focus.org:3".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: true,
                },
            }],
        };
        assert_eq!(
            invalid_explore_unique.validation_error().as_deref(),
            Some(
                "workflow step 0 is invalid: explore unique is only supported for the structure lens"
            )
        );
    }

    #[test]
    fn built_in_workflows_are_valid_named_specs() {
        let workflows = built_in_workflows();
        assert_eq!(workflows.len(), 5);

        let ids: Vec<&str> = workflows
            .iter()
            .map(|workflow| workflow.metadata.workflow_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
                BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
                BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID,
                BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID,
                BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID,
            ]
        );

        for workflow in &workflows {
            assert_eq!(workflow.validation_error(), None);
            assert!(
                !workflow.inputs.is_empty(),
                "built-in workflow {} should declare inputs",
                workflow.metadata.workflow_id
            );
        }
        assert_eq!(workflows[0].inputs[0].kind, WorkflowInputKind::FocusTarget);
        assert_eq!(workflows[1].inputs[0].kind, WorkflowInputKind::FocusTarget);
        assert_eq!(workflows[2].inputs[0].kind, WorkflowInputKind::FocusTarget);
        assert_eq!(workflows[3].inputs[0].kind, WorkflowInputKind::FocusTarget);
        assert_eq!(workflows[4].inputs[0].kind, WorkflowInputKind::NoteTarget);

        assert_eq!(
            built_in_workflow(BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID),
            Some(workflows[0].clone())
        );
        assert_eq!(built_in_workflow("workflow/builtin/missing"), None);

        let summaries = built_in_workflow_summaries();
        assert_eq!(summaries.len(), workflows.len());
        assert_eq!(summaries[0].step_count, workflows[0].steps.len());
        assert_eq!(
            summaries[4].metadata.workflow_id,
            BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID
        );
    }

    #[test]
    fn built_in_workflows_round_trip_with_input_backed_resolve_targets() {
        let workflow = built_in_workflow(BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID)
            .expect("built-in unresolved sweep should exist");
        let serialized =
            serde_json::to_value(&workflow).expect("built-in workflow should serialize");
        assert_eq!(serialized["inputs"][0]["kind"], json!("focus-target"));
        assert_eq!(serialized["steps"][0]["kind"], json!("resolve"));
        assert_eq!(serialized["steps"][0]["target"]["kind"], json!("input"));
        assert_eq!(serialized["steps"][0]["target"]["input_id"], json!("focus"));
        assert_eq!(serialized["steps"][2]["kind"], json!("explore"));
        assert_eq!(serialized["steps"][2]["focus"]["kind"], json!("input"));
        assert_eq!(serialized["steps"][2]["focus"]["input_id"], json!("focus"));

        let round_trip: WorkflowSpec =
            serde_json::from_value(serialized).expect("built-in workflow should deserialize");
        assert_eq!(round_trip, workflow);

        let periodic = built_in_workflow(BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID)
            .expect("periodic review workflow should exist");
        assert_eq!(periodic.steps.len(), 6);
        assert_eq!(periodic.steps[1].step_id, "review-unresolved");
        assert_eq!(periodic.steps[4].step_id, "review-refs");
        assert_eq!(periodic.validation_error(), None);

        let weak = built_in_workflow(BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID)
            .expect("weak integration review workflow should exist");
        assert_eq!(weak.steps.len(), 4);
        assert_eq!(weak.steps[1].step_id, "review-weak-integration");
        assert_eq!(weak.validation_error(), None);
    }

    #[test]
    fn workflow_specs_reject_invalid_inputs_and_missing_input_targets() {
        let duplicate_inputs = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/duplicate-inputs".to_owned(),
                title: "Duplicate Inputs".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: vec![
                WorkflowInputSpec {
                    input_id: "focus".to_owned(),
                    title: "Focus".to_owned(),
                    summary: None,
                    kind: WorkflowInputKind::NoteTarget,
                },
                WorkflowInputSpec {
                    input_id: "focus".to_owned(),
                    title: "Focus Again".to_owned(),
                    summary: None,
                    kind: WorkflowInputKind::NoteTarget,
                },
            ],
            steps: vec![WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            }],
        };
        assert_eq!(
            duplicate_inputs.validation_error().as_deref(),
            Some("workflow input 1 reuses duplicate input_id focus")
        );

        let missing_input_target = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/missing-input".to_owned(),
                title: "Missing Input".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![WorkflowStepSpec {
                step_id: "resolve-focus".to_owned(),
                payload: WorkflowStepPayload::Resolve {
                    target: WorkflowResolveTarget::Input {
                        input_id: "focus".to_owned(),
                    },
                },
            }],
        };
        assert_eq!(
            missing_input_target.validation_error().as_deref(),
            Some("workflow step 0 is invalid: target must reference a declared workflow input")
        );

        let missing_focus_input = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/missing-focus-input".to_owned(),
                title: "Missing Focus Input".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![WorkflowStepSpec {
                step_id: "explore-focus".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: false,
                },
            }],
        };
        assert_eq!(
            missing_focus_input.validation_error().as_deref(),
            Some("workflow step 0 is invalid: focus must reference a declared workflow input")
        );

        let note_target_used_as_focus_input = WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: "workflow/note-focus-mismatch".to_owned(),
                title: "Note Focus Mismatch".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: vec![WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus".to_owned(),
                summary: None,
                kind: WorkflowInputKind::NoteTarget,
            }],
            steps: vec![WorkflowStepSpec {
                step_id: "explore-focus".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: WorkflowExploreFocus::Input {
                        input_id: "focus".to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 25,
                    unique: false,
                },
            }],
        };
        assert_eq!(
            note_target_used_as_focus_input
                .validation_error()
                .as_deref(),
            Some("workflow step 0 is invalid: focus must reference a declared focus-target input")
        );
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
