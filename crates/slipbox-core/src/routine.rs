use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::{
    audit::{CorpusAuditEntry, CorpusAuditKind, CorpusAuditResult},
    reports::{ReportJsonlLineKind, ReportProfileSpec, ReportProfileSubject},
    review::{
        ReviewFinding, ReviewFindingPair, ReviewFindingStatusDiff, ReviewRunDiff, ReviewRunSummary,
    },
    validation::{
        default_audit_limit, validate_optional_report_profile_id_field,
        validate_optional_review_id_field, validate_optional_text_field,
        validate_required_text_field, validate_review_routine_compare_policy,
        validate_review_routine_id_field, validate_review_routine_inputs,
        validate_review_routine_report_profiles, validate_workflow_id_field,
    },
    workflow::{
        BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID, BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID,
        BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID, WorkflowCatalogIssue, WorkflowExecutionResult,
        WorkflowInputAssignment, WorkflowInputKind, WorkflowInputSpec, WorkflowStepReport,
        WorkflowSummary,
    },
};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoutineSummary {
    #[serde(flatten)]
    pub metadata: ReviewRoutineMetadata,
    pub source_kind: ReviewRoutineSourceKind,
    pub input_count: usize,
    pub report_profile_count: usize,
}

impl From<&ReviewRoutineSpec> for ReviewRoutineSummary {
    fn from(routine: &ReviewRoutineSpec) -> Self {
        Self {
            metadata: routine.metadata.clone(),
            source_kind: routine.source.kind(),
            input_count: routine.inputs.len(),
            report_profile_count: routine.report_profile_ids.len(),
        }
    }
}

#[must_use]
pub fn built_in_review_routines() -> Vec<ReviewRoutineSpec> {
    static ROUTINES: LazyLock<Vec<ReviewRoutineSpec>> = LazyLock::new(|| {
        vec![
            built_in_context_sweep_review_routine(),
            built_in_duplicate_title_review_routine(),
        ]
    });
    ROUTINES.clone()
}

#[must_use]
pub fn built_in_review_routine(routine_id: &str) -> Option<ReviewRoutineSpec> {
    built_in_review_routines()
        .into_iter()
        .find(|routine| routine.metadata.routine_id == routine_id)
}

#[must_use]
pub fn built_in_review_routine_summaries() -> Vec<ReviewRoutineSummary> {
    built_in_review_routines()
        .into_iter()
        .map(|routine| ReviewRoutineSummary::from(&routine))
        .collect()
}

fn built_in_context_sweep_review_routine() -> ReviewRoutineSpec {
    ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: BUILT_IN_REVIEW_ROUTINE_CONTEXT_SWEEP_ID.to_owned(),
            title: "Context Sweep Review".to_owned(),
            summary: Some(
                "Run the built-in context sweep workflow as a durable review routine.".to_owned(),
            ),
        },
        source: ReviewRoutineSource::Workflow {
            workflow_id: BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID.to_owned(),
        },
        inputs: vec![WorkflowInputSpec {
            input_id: "focus".to_owned(),
            title: "Focus target".to_owned(),
            summary: Some("Note or anchor target to review for contextual pressure".to_owned()),
            kind: WorkflowInputKind::FocusTarget,
        }],
        save_review: ReviewRoutineSaveReviewPolicy::default(),
        compare: None,
        report_profile_ids: Vec::new(),
    }
}

fn built_in_duplicate_title_review_routine() -> ReviewRoutineSpec {
    ReviewRoutineSpec {
        metadata: ReviewRoutineMetadata {
            routine_id: BUILT_IN_REVIEW_ROUTINE_DUPLICATE_TITLE_ID.to_owned(),
            title: "Duplicate Title Review".to_owned(),
            summary: Some("Run the duplicate-title audit as a durable review routine.".to_owned()),
        },
        source: ReviewRoutineSource::Audit {
            audit: CorpusAuditKind::DuplicateTitles,
            limit: 200,
        },
        inputs: Vec::new(),
        save_review: ReviewRoutineSaveReviewPolicy::default(),
        compare: None,
        report_profile_ids: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoutineIdParams {
    pub routine_id: String,
}

impl ReviewRoutineIdParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_routine_id_field(&self.routine_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListReviewRoutinesParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoutineResult {
    pub routine: ReviewRoutineSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListReviewRoutinesResult {
    pub routines: Vec<ReviewRoutineSummary>,
    #[serde(default)]
    pub issues: Vec<WorkflowCatalogIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunReviewRoutineParams {
    pub routine_id: String,
    #[serde(default)]
    pub inputs: Vec<WorkflowInputAssignment>,
}

impl RunReviewRoutineParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_routine_id_field(&self.routine_id).or_else(|| {
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
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReviewRoutineSourceExecutionResult {
    Audit {
        result: Box<CorpusAuditResult>,
    },
    Workflow {
        result: Box<WorkflowExecutionResult>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReviewRoutineReportLine {
    Routine {
        routine: ReviewRoutineSummary,
    },
    Workflow {
        workflow: WorkflowSummary,
    },
    Step {
        step: Box<WorkflowStepReport>,
    },
    Audit {
        audit: CorpusAuditKind,
    },
    Entry {
        entry: Box<CorpusAuditEntry>,
    },
    Review {
        review: ReviewRunSummary,
    },
    Finding {
        finding: Box<ReviewFinding>,
    },
    Diff {
        base_review: ReviewRunSummary,
        target_review: ReviewRunSummary,
    },
    Added {
        finding: Box<ReviewFinding>,
    },
    Removed {
        finding: Box<ReviewFinding>,
    },
    Unchanged {
        finding: Box<ReviewFindingPair>,
    },
    ContentChanged {
        finding: Box<ReviewFindingPair>,
    },
    StatusChanged {
        change: Box<ReviewFindingStatusDiff>,
    },
}

impl ReviewRoutineReportLine {
    #[must_use]
    pub fn line_kind(&self) -> ReportJsonlLineKind {
        match self {
            Self::Routine { .. } => ReportJsonlLineKind::Routine,
            Self::Workflow { .. } => ReportJsonlLineKind::Workflow,
            Self::Step { .. } => ReportJsonlLineKind::Step,
            Self::Audit { .. } => ReportJsonlLineKind::Audit,
            Self::Entry { .. } => ReportJsonlLineKind::Entry,
            Self::Review { .. } => ReportJsonlLineKind::Review,
            Self::Finding { .. } => ReportJsonlLineKind::Finding,
            Self::Diff { .. } => ReportJsonlLineKind::Diff,
            Self::Added { .. } => ReportJsonlLineKind::Added,
            Self::Removed { .. } => ReportJsonlLineKind::Removed,
            Self::Unchanged { .. } => ReportJsonlLineKind::Unchanged,
            Self::ContentChanged { .. } => ReportJsonlLineKind::ContentChanged,
            Self::StatusChanged { .. } => ReportJsonlLineKind::StatusChanged,
        }
    }

    #[must_use]
    pub const fn subject(&self) -> ReportProfileSubject {
        match self {
            Self::Routine { .. } => ReportProfileSubject::Routine,
            Self::Workflow { .. } | Self::Step { .. } => ReportProfileSubject::Workflow,
            Self::Audit { .. } | Self::Entry { .. } => ReportProfileSubject::Audit,
            Self::Review { .. } | Self::Finding { .. } => ReportProfileSubject::Review,
            Self::Diff { .. }
            | Self::Added { .. }
            | Self::Removed { .. }
            | Self::Unchanged { .. }
            | Self::ContentChanged { .. }
            | Self::StatusChanged { .. } => ReportProfileSubject::Diff,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedReportProfile {
    pub profile: ReportProfileSpec,
    pub lines: Vec<ReviewRoutineReportLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoutineCompareResult {
    pub target: ReviewRoutineCompareTarget,
    #[serde(default)]
    pub base_review: Option<ReviewRunSummary>,
    #[serde(default)]
    pub diff: Option<Box<ReviewRunDiff>>,
    #[serde(default)]
    pub report: Option<AppliedReportProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoutineExecutionResult {
    pub routine: ReviewRoutineSummary,
    pub source: ReviewRoutineSourceExecutionResult,
    #[serde(default)]
    pub saved_review: Option<ReviewRunSummary>,
    #[serde(default)]
    pub compare: Option<ReviewRoutineCompareResult>,
    #[serde(default)]
    pub reports: Vec<AppliedReportProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunReviewRoutineResult {
    pub result: ReviewRoutineExecutionResult,
}
