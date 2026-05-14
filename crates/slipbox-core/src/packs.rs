use serde::{Deserialize, Serialize};

use crate::{
    reports::ReportProfileSpec,
    routine::ReviewRoutineSpec,
    validation::{
        validate_optional_text_field, validate_required_text_field,
        validate_workbench_pack_id_field, validate_workbench_pack_manifest,
    },
    workflow::{WorkflowCatalogIssue, WorkflowSpec},
};

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
    #[serde(default)]
    pub issues: Vec<WorkflowCatalogIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteWorkbenchPackResult {
    pub pack_id: String,
}
