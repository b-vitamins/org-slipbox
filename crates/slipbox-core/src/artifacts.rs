use serde::{Deserialize, Serialize};

use crate::{
    exploration::{
        CompareNotesParams, ExplorationLens, ExploreParams, ExploreResult, NoteComparisonGroup,
        NoteComparisonResult,
    },
    nodes::NodeRecord,
    validation::{
        default_artifact_overwrite, default_backlink_limit, validate_artifact_id_field,
        validate_optional_text_field, validate_required_text_field,
    },
};

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
