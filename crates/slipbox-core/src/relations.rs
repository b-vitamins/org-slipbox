use serde::{Deserialize, Serialize};

use crate::{
    nodes::{AnchorRecord, NodeRecord},
    validation::{
        default_agenda_limit, default_backlink_limit, default_graph_max_title_length,
        default_ref_limit, default_tag_limit,
    },
};

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
