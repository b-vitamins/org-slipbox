use serde::{Deserialize, Serialize};

use crate::{
    nodes::{AnchorRecord, NodeRecord},
    relations::{
        BacklinkRecord, ExplorationExplanation, ForwardLinkRecord, PlanningField, ReflinkRecord,
        UnlinkedReferenceRecord,
    },
    validation::default_backlink_limit,
};

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
