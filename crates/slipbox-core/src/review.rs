use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    audit::{CorpusAuditEntry, CorpusAuditKind, CorpusAuditParams, CorpusAuditResult},
    nodes::{AnchorRecord, NodeRecord},
    validation::{
        default_audit_limit, default_review_overwrite, validate_optional_text_field,
        validate_positive_position, validate_required_text_field, validate_review_finding_id_field,
        validate_review_finding_matches_run, validate_review_id_field,
        validate_review_node_key_set, validate_structural_write_file_path_field,
        validate_workflow_id_field, validate_workflow_review_inputs,
        validate_workflow_review_source,
    },
    workflow::{
        RunWorkflowParams, WorkflowExecutionResult, WorkflowInputAssignment, WorkflowStepReport,
        WorkflowSummary,
    },
    write::{StructuralWriteAffectedFiles, StructuralWriteIndexRefreshStatus},
};

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
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AuditRemediationPreviewIdentity {
    DanglingLink {
        source_node_key: String,
        missing_explicit_id: String,
        file_path: String,
        line: u32,
        column: u32,
        preview: String,
    },
    DuplicateTitle {
        title: String,
        node_keys: Vec<String>,
    },
}

impl AuditRemediationPreviewIdentity {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::DanglingLink {
                source_node_key,
                missing_explicit_id,
                file_path,
                line,
                column,
                preview,
            } => validate_required_text_field(source_node_key, "source_node_key")
                .or_else(|| {
                    validate_required_text_field(missing_explicit_id, "missing_explicit_id")
                })
                .or_else(|| validate_structural_write_file_path_field(file_path, "file_path"))
                .or_else(|| validate_positive_position(*line, "line"))
                .or_else(|| validate_positive_position(*column, "column"))
                .or_else(|| validate_required_text_field(preview, "preview")),
            Self::DuplicateTitle { title, node_keys } => {
                validate_required_text_field(title, "title")
                    .or_else(|| validate_review_node_key_set(node_keys, "node_keys", 2))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AuditRemediationApplyAction {
    UnlinkDanglingLink {
        source_node_key: String,
        missing_explicit_id: String,
        file_path: String,
        line: u32,
        column: u32,
        preview: String,
        replacement_text: String,
    },
}

impl AuditRemediationApplyAction {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::UnlinkDanglingLink {
                source_node_key,
                missing_explicit_id,
                file_path,
                line,
                column,
                preview,
                replacement_text,
            } => validate_required_text_field(source_node_key, "source_node_key")
                .or_else(|| {
                    validate_required_text_field(missing_explicit_id, "missing_explicit_id")
                })
                .or_else(|| validate_structural_write_file_path_field(file_path, "file_path"))
                .or_else(|| validate_positive_position(*line, "line"))
                .or_else(|| validate_positive_position(*column, "column"))
                .or_else(|| validate_required_text_field(preview, "preview"))
                .or_else(|| validate_required_text_field(replacement_text, "replacement_text"))
                .or_else(|| {
                    replacement_text
                        .contains(['\n', '\r'])
                        .then(|| "replacement_text must be a single line".to_owned())
                }),
        }
    }

    #[must_use]
    pub fn preview_identity(&self) -> AuditRemediationPreviewIdentity {
        match self {
            Self::UnlinkDanglingLink {
                source_node_key,
                missing_explicit_id,
                file_path,
                line,
                column,
                preview,
                ..
            } => AuditRemediationPreviewIdentity::DanglingLink {
                source_node_key: source_node_key.clone(),
                missing_explicit_id: missing_explicit_id.clone(),
                file_path: file_path.clone(),
                line: *line,
                column: *column,
                preview: preview.clone(),
            },
        }
    }

    #[must_use]
    pub fn affected_file(&self) -> &str {
        match self {
            Self::UnlinkDanglingLink { file_path, .. } => file_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingRemediationPreview {
    pub review_id: String,
    pub finding_id: String,
    pub status: ReviewFindingStatus,
    pub preview_identity: AuditRemediationPreviewIdentity,
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

        let preview_identity = AuditRemediationPreviewIdentity::from(&payload);
        Ok(Self {
            review_id: review_id.to_owned(),
            finding_id: finding.finding_id.clone(),
            status: finding.status,
            preview_identity,
            payload,
        })
    }
}

impl From<&AuditRemediationPreviewPayload> for AuditRemediationPreviewIdentity {
    fn from(payload: &AuditRemediationPreviewPayload) -> Self {
        match payload {
            AuditRemediationPreviewPayload::DanglingLink {
                source,
                missing_explicit_id,
                file_path,
                line,
                column,
                preview,
                ..
            } => Self::DanglingLink {
                source_node_key: source.node_key.clone(),
                missing_explicit_id: missing_explicit_id.clone(),
                file_path: file_path.clone(),
                line: *line,
                column: *column,
                preview: preview.clone(),
            },
            AuditRemediationPreviewPayload::DuplicateTitle { title, notes, .. } => {
                Self::DuplicateTitle {
                    title: title.clone(),
                    node_keys: notes.iter().map(|note| note.node_key.clone()).collect(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingRemediationApplyParams {
    pub review_id: String,
    pub finding_id: String,
    pub expected_preview: AuditRemediationPreviewIdentity,
    pub action: AuditRemediationApplyAction,
}

impl ReviewFindingRemediationApplyParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_id_field(&self.review_id)
            .or_else(|| validate_review_finding_id_field(&self.finding_id))
            .or_else(|| self.expected_preview.validation_error())
            .or_else(|| self.action.validation_error())
            .or_else(|| {
                (self.action.preview_identity() != self.expected_preview).then(|| {
                    "remediation action must match the expected preview identity".to_owned()
                })
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingRemediationApplication {
    pub review_id: String,
    pub finding_id: String,
    pub preview_identity: AuditRemediationPreviewIdentity,
    pub action: AuditRemediationApplyAction,
    #[serde(flatten)]
    pub affected_files: StructuralWriteAffectedFiles,
    pub index_refresh: StructuralWriteIndexRefreshStatus,
}

impl ReviewFindingRemediationApplication {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_review_id_field(&self.review_id)
            .or_else(|| validate_review_finding_id_field(&self.finding_id))
            .or_else(|| self.preview_identity.validation_error())
            .or_else(|| self.action.validation_error())
            .or_else(|| {
                (self.action.preview_identity() != self.preview_identity).then(|| {
                    "remediation application action must match the preview identity".to_owned()
                })
            })
            .or_else(|| self.affected_files.validation_error())
            .or_else(|| {
                (!self
                    .affected_files
                    .changed_files
                    .iter()
                    .any(|file| file == self.action.affected_file()))
                .then(|| {
                    format!(
                        "remediation application affected files must include changed file {}",
                        self.action.affected_file()
                    )
                })
            })
            .or_else(|| {
                (!self.affected_files.removed_files.is_empty()).then(|| {
                    "remediation applications must not remove files for supported actions"
                        .to_owned()
                })
            })
            .or_else(|| {
                (self.index_refresh != StructuralWriteIndexRefreshStatus::Refreshed).then(|| {
                    "remediation applications must be returned after index refresh".to_owned()
                })
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
pub struct ReviewFindingRemediationApplyResult {
    pub application: ReviewFindingRemediationApplication,
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
