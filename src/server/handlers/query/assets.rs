use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slipbox_core::{
    AppliedReportProfile, CompareNotesParams, CorpusAuditParams, DeleteWorkbenchPackResult,
    ExecutedExplorationArtifact, ExplorationArtifactPayload, ExplorationArtifactSummary,
    ExplorationLens, ExploreParams, ExploreResult, ImportWorkbenchPackParams,
    ImportWorkbenchPackResult, ListReviewRoutinesParams, ListReviewRoutinesResult,
    ListWorkbenchPacksParams, ListWorkbenchPacksResult, ListWorkflowsParams, ListWorkflowsResult,
    NodeRecord, NoteComparisonGroup, NoteComparisonResult, ReportProfileMode, ReportProfileSpec,
    ReviewFinding, ReviewFindingPayload, ReviewFindingStatus, ReviewRoutineCompareResult,
    ReviewRoutineExecutionResult, ReviewRoutineIdParams, ReviewRoutineReportLine,
    ReviewRoutineResult, ReviewRoutineSource, ReviewRoutineSourceExecutionResult,
    ReviewRoutineSpec, ReviewRun, ReviewRunDiff, ReviewRunDiffBucket, ReviewRunMetadata,
    ReviewRunPayload, ReviewRunSummary, RunReviewRoutineParams, RunReviewRoutineResult,
    RunWorkflowParams, RunWorkflowResult, SaveCorpusAuditReviewParams, SaveWorkflowReviewParams,
    SaveWorkflowReviewResult, SavedComparisonArtifact, SavedExplorationArtifact,
    SavedLensViewArtifact, ValidateWorkbenchPackParams, ValidateWorkbenchPackResult,
    WorkbenchPackCompatibilityEnvelope, WorkbenchPackIdParams, WorkbenchPackIssue,
    WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackResult, WorkbenchPackSummary,
    WorkflowExecutionResult, WorkflowIdParams, WorkflowInputAssignment, WorkflowResolveTarget,
    WorkflowResult, WorkflowSpec, WorkflowStepPayload, WorkflowStepReport,
    WorkflowStepReportPayload,
};
use slipbox_rpc::JsonRpcError;

use super::common::{
    invalid_request, validate_pack_id_params, validate_review_routine_id_params,
    validate_workflow_id_params, with_step_context,
};
use super::exploration::{
    execute_compare_notes_query, execute_explore_query, execute_saved_exploration_artifact_by_id,
    save_exploration_artifact_with_policy,
};
use super::reviews::{
    execute_corpus_audit_query, review_from_audit_result, save_review_run_with_policy,
};
use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;
use crate::server::workflows::{WorkflowCatalog, discover_workflow_catalog};

fn discover_server_workflow_catalog(state: &ServerState) -> Result<WorkflowCatalog, JsonRpcError> {
    let packs = state
        .database
        .list_workbench_packs()
        .map_err(|error| internal_error(error.context("failed to list workbench packs")))?;
    Ok(discover_workflow_catalog(
        &state.root,
        &state.workflow_dirs,
        &packs,
    ))
}

fn workflow_lens_accepts_anchor_focus(lens: ExplorationLens) -> bool {
    matches!(
        lens,
        ExplorationLens::Refs | ExplorationLens::Time | ExplorationLens::Tasks
    )
}

fn resolve_workflow_note_target(
    state: &mut ServerState,
    target: &WorkflowResolveTarget,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    match target {
        WorkflowResolveTarget::Id { id } => state
            .database
            .node_from_id(id)
            .map_err(|error| {
                internal_error(error.context(format!("failed to resolve {description}")))
            })?
            .ok_or_else(|| invalid_request(format!("unknown {description}: {id}"))),
        WorkflowResolveTarget::Title { title } => {
            let matches = state
                .database
                .node_from_title_or_alias(title, false)
                .map_err(|error| {
                    internal_error(error.context(format!("failed to resolve {description}")))
                })?;
            if matches.len() > 1 {
                return Err(invalid_request(format!("multiple nodes match {title}")));
            }
            matches
                .into_iter()
                .next()
                .ok_or_else(|| invalid_request(format!("unknown {description}: {title}")))
        }
        WorkflowResolveTarget::Reference { reference } => state
            .database
            .node_from_ref(reference)
            .map_err(|error| {
                internal_error(error.context(format!("failed to resolve {description}")))
            })?
            .ok_or_else(|| invalid_request(format!("unknown {description}: {reference}"))),
        WorkflowResolveTarget::NodeKey { node_key } => state.known_note(node_key, description),
        WorkflowResolveTarget::Input { .. } => Err(internal_error(anyhow::anyhow!(
            "workflow input reference reached runtime resolution unexpectedly"
        ))),
    }
}

fn resolve_workflow_note_target_from_focus(
    state: &mut ServerState,
    target: &WorkflowResolveTarget,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    match target {
        WorkflowResolveTarget::NodeKey { node_key } => {
            state.known_note_for_node_or_anchor(node_key, description)
        }
        WorkflowResolveTarget::Id { .. }
        | WorkflowResolveTarget::Title { .. }
        | WorkflowResolveTarget::Reference { .. } => {
            resolve_workflow_note_target(state, target, description)
        }
        WorkflowResolveTarget::Input { .. } => Err(internal_error(anyhow::anyhow!(
            "workflow input reference reached runtime note resolution unexpectedly"
        ))),
    }
}

fn resolve_workflow_focus_target(
    state: &mut ServerState,
    target: &WorkflowResolveTarget,
    lens: ExplorationLens,
    description: &str,
) -> Result<String, JsonRpcError> {
    match target {
        WorkflowResolveTarget::NodeKey { node_key } if workflow_lens_accepts_anchor_focus(lens) => {
            if state
                .database
                .anchor_by_key(node_key)
                .map_err(|error| {
                    internal_error(error.context(format!("failed to resolve {description}")))
                })?
                .is_some()
            {
                Ok(node_key.clone())
            } else {
                state
                    .known_note(node_key, description)
                    .map(|note| note.node_key)
            }
        }
        WorkflowResolveTarget::NodeKey { node_key } => state
            .known_note_for_node_or_anchor(node_key, description)
            .map(|note| note.node_key),
        WorkflowResolveTarget::Id { .. }
        | WorkflowResolveTarget::Title { .. }
        | WorkflowResolveTarget::Reference { .. } => {
            resolve_workflow_note_target(state, target, description).map(|note| note.node_key)
        }
        WorkflowResolveTarget::Input { .. } => Err(internal_error(anyhow::anyhow!(
            "workflow input reference reached runtime focus resolution unexpectedly"
        ))),
    }
}
fn save_workbench_pack_with_policy(
    state: &mut ServerState,
    pack: &WorkbenchPackManifest,
    overwrite: bool,
) -> Result<WorkbenchPackSummary, JsonRpcError> {
    if overwrite {
        state
            .database
            .save_workbench_pack(pack)
            .map_err(|error| internal_error(error.context("failed to save workbench pack")))?;
    } else if !state
        .database
        .save_workbench_pack_if_absent(pack)
        .map_err(|error| {
            internal_error(error.context("failed to save workbench pack without overwrite"))
        })?
    {
        return Err(invalid_request(format!(
            "workbench pack already exists: {}",
            pack.metadata.pack_id
        )));
    }

    Ok(WorkbenchPackSummary::from(pack))
}
fn stable_json_fingerprint<T: Serialize>(value: &T) -> Result<String, JsonRpcError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        internal_error(anyhow::anyhow!(
            "failed to serialize review source: {error}"
        ))
    })?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{hash:016x}"))
}
fn generated_workflow_review_id(params: &SaveWorkflowReviewParams) -> Result<String, JsonRpcError> {
    if params.inputs.is_empty() {
        return Ok(format!("review/{}", params.workflow_id));
    }

    let mut inputs = params.inputs.clone();
    inputs.sort_by(|left, right| left.input_id.cmp(&right.input_id));
    let fingerprint = stable_json_fingerprint(&inputs)?;
    Ok(format!(
        "review/{}/inputs-{fingerprint}",
        params.workflow_id
    ))
}

fn intended_workflow_review_id(params: &SaveWorkflowReviewParams) -> Result<String, JsonRpcError> {
    params
        .review_id
        .clone()
        .map(Ok)
        .unwrap_or_else(|| generated_workflow_review_id(params))
}

fn reject_existing_review_run(state: &ServerState, review_id: &str) -> Result<(), JsonRpcError> {
    if state
        .database
        .review_run(review_id)
        .map_err(|error| internal_error(error.context("failed to load review run")))?
        .is_some()
    {
        return Err(invalid_request(format!(
            "review run already exists: {review_id}"
        )));
    }

    Ok(())
}
fn review_from_workflow_result(
    params: &SaveWorkflowReviewParams,
    result: &WorkflowExecutionResult,
    review_id: String,
) -> Result<ReviewRun, JsonRpcError> {
    let metadata = ReviewRunMetadata {
        review_id,
        title: params
            .title
            .clone()
            .unwrap_or_else(|| format!("{} Review", result.workflow.metadata.title)),
        summary: params.summary.clone().or_else(|| {
            Some(format!(
                "{} step findings from workflow {}",
                result.steps.len(),
                result.workflow.metadata.workflow_id
            ))
        }),
    };
    let review = ReviewRun {
        metadata,
        payload: ReviewRunPayload::Workflow {
            workflow: result.workflow.clone(),
            inputs: params.inputs.clone(),
            step_ids: result
                .steps
                .iter()
                .map(|step| step.step_id.clone())
                .collect(),
        },
        findings: result
            .steps
            .iter()
            .map(|step| ReviewFinding {
                finding_id: format!("workflow-step/{}", step.step_id),
                status: ReviewFindingStatus::Open,
                payload: ReviewFindingPayload::WorkflowStep {
                    step: Box::new(step.clone()),
                },
            })
            .collect(),
    };
    if let Some(message) = review.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(review)
}

fn generated_routine_review_id(
    routine: &ReviewRoutineSpec,
    inputs: &[WorkflowInputAssignment],
) -> Result<String, JsonRpcError> {
    if inputs.is_empty() {
        return Ok(format!("review/{}", routine.metadata.routine_id));
    }

    let mut inputs = inputs.to_vec();
    inputs.sort_by(|left, right| left.input_id.cmp(&right.input_id));
    let fingerprint = stable_json_fingerprint(&inputs)?;
    Ok(format!(
        "review/{}/inputs-{fingerprint}",
        routine.metadata.routine_id
    ))
}

fn intended_routine_review_id(
    routine: &ReviewRoutineSpec,
    inputs: &[WorkflowInputAssignment],
) -> Result<String, JsonRpcError> {
    routine
        .save_review
        .review_id
        .clone()
        .map(Ok)
        .unwrap_or_else(|| generated_routine_review_id(routine, inputs))
}

fn validate_review_routine_input_assignments(
    routine: &ReviewRoutineSpec,
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
        if !routine
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

    routine
        .inputs
        .iter()
        .find(|input| !seen_assignments.contains(&input.input_id.as_str()))
        .map(|input| format!("workflow input {} must be assigned", input.input_id))
}

fn review_from_routine_source_result(
    routine: &ReviewRoutineSpec,
    inputs: &[WorkflowInputAssignment],
    source: &ReviewRoutineSourceExecutionResult,
    review_id: String,
) -> Result<ReviewRun, JsonRpcError> {
    match source {
        ReviewRoutineSourceExecutionResult::Audit { result } => review_from_audit_result(
            &SaveCorpusAuditReviewParams {
                audit: result.audit,
                limit: match &routine.source {
                    ReviewRoutineSource::Audit { limit, .. } => *limit,
                    _ => 0,
                },
                review_id: Some(review_id),
                title: routine.save_review.title.clone(),
                summary: routine.save_review.summary.clone(),
                overwrite: routine.save_review.overwrite,
            },
            result,
        ),
        ReviewRoutineSourceExecutionResult::Workflow { result } => review_from_workflow_result(
            &SaveWorkflowReviewParams {
                workflow_id: result.workflow.metadata.workflow_id.clone(),
                inputs: inputs.to_vec(),
                review_id: Some(review_id.clone()),
                title: routine.save_review.title.clone(),
                summary: routine.save_review.summary.clone(),
                overwrite: routine.save_review.overwrite,
            },
            result,
            review_id,
        ),
    }
}

fn latest_compatible_review_run(
    state: &ServerState,
    target: &ReviewRun,
) -> Result<Option<ReviewRun>, JsonRpcError> {
    let reviews = state
        .database
        .list_review_runs_newest_first()
        .map_err(|error| internal_error(error.context("failed to list review runs")))?;
    Ok(reviews
        .into_iter()
        .find(|review| ReviewRunDiff::between(review, target).is_ok()))
}

fn report_line_status(line: &ReviewRoutineReportLine) -> Option<ReviewFindingStatus> {
    match line {
        ReviewRoutineReportLine::Finding { finding }
        | ReviewRoutineReportLine::Added { finding }
        | ReviewRoutineReportLine::Removed { finding } => Some(finding.status),
        ReviewRoutineReportLine::Unchanged { finding }
        | ReviewRoutineReportLine::ContentChanged { finding } => Some(finding.target.status),
        ReviewRoutineReportLine::StatusChanged { change } => Some(change.to_status),
        _ => None,
    }
}

fn report_line_bucket(line: &ReviewRoutineReportLine) -> Option<ReviewRunDiffBucket> {
    match line {
        ReviewRoutineReportLine::Added { .. } => Some(ReviewRunDiffBucket::Added),
        ReviewRoutineReportLine::Removed { .. } => Some(ReviewRunDiffBucket::Removed),
        ReviewRoutineReportLine::Unchanged { .. } => Some(ReviewRunDiffBucket::Unchanged),
        ReviewRoutineReportLine::ContentChanged { .. } => Some(ReviewRunDiffBucket::ContentChanged),
        ReviewRoutineReportLine::StatusChanged { .. } => Some(ReviewRunDiffBucket::StatusChanged),
        _ => None,
    }
}

fn report_line_matches_profile(
    profile: &ReportProfileSpec,
    line: &ReviewRoutineReportLine,
) -> bool {
    let line_kind = line.line_kind();
    if !profile
        .subjects
        .iter()
        .any(|subject| subject.supports_line_kind(&line_kind))
    {
        return false;
    }

    if matches!(profile.mode, ReportProfileMode::Summary) && line_kind.is_detail_line() {
        return false;
    }
    if let Some(line_kinds) = &profile.jsonl_line_kinds
        && !line_kinds.contains(&line_kind)
    {
        return false;
    }
    if let Some(status_filters) = &profile.status_filters
        && let Some(status) = report_line_status(line)
        && !status_filters.contains(&status)
    {
        return false;
    }
    if let Some(diff_buckets) = &profile.diff_buckets
        && let Some(bucket) = report_line_bucket(line)
        && !diff_buckets.contains(&bucket)
    {
        return false;
    }

    true
}

fn apply_report_profile(
    profile: &ReportProfileSpec,
    routine: &ReviewRoutineSpec,
    source: &ReviewRoutineSourceExecutionResult,
    review: Option<&ReviewRun>,
    diff: Option<&ReviewRunDiff>,
) -> AppliedReportProfile {
    let routine_summary = routine.into();
    let mut candidates = vec![ReviewRoutineReportLine::Routine {
        routine: routine_summary,
    }];

    match source {
        ReviewRoutineSourceExecutionResult::Audit { result } => {
            candidates.push(ReviewRoutineReportLine::Audit {
                audit: result.audit,
            });
            candidates.extend(result.entries.iter().cloned().map(|entry| {
                ReviewRoutineReportLine::Entry {
                    entry: Box::new(entry),
                }
            }));
        }
        ReviewRoutineSourceExecutionResult::Workflow { result } => {
            candidates.push(ReviewRoutineReportLine::Workflow {
                workflow: result.workflow.clone(),
            });
            candidates.extend(result.steps.iter().cloned().map(|step| {
                ReviewRoutineReportLine::Step {
                    step: Box::new(step),
                }
            }));
        }
    }

    if let Some(review) = review {
        candidates.push(ReviewRoutineReportLine::Review {
            review: ReviewRunSummary::from(review),
        });
        candidates.extend(review.findings.iter().cloned().map(|finding| {
            ReviewRoutineReportLine::Finding {
                finding: Box::new(finding),
            }
        }));
    }

    if let Some(diff) = diff {
        candidates.push(ReviewRoutineReportLine::Diff {
            base_review: diff.base_review.clone(),
            target_review: diff.target_review.clone(),
        });
        candidates.extend(diff.added.iter().cloned().map(|finding| {
            ReviewRoutineReportLine::Added {
                finding: Box::new(finding),
            }
        }));
        candidates.extend(diff.removed.iter().cloned().map(|finding| {
            ReviewRoutineReportLine::Removed {
                finding: Box::new(finding),
            }
        }));
        candidates.extend(diff.unchanged.iter().cloned().map(|finding| {
            ReviewRoutineReportLine::Unchanged {
                finding: Box::new(finding),
            }
        }));
        candidates.extend(diff.content_changed.iter().cloned().map(|finding| {
            ReviewRoutineReportLine::ContentChanged {
                finding: Box::new(finding),
            }
        }));
        candidates.extend(diff.status_changed.iter().cloned().map(|change| {
            ReviewRoutineReportLine::StatusChanged {
                change: Box::new(change),
            }
        }));
    }

    AppliedReportProfile {
        profile: profile.clone(),
        lines: candidates
            .into_iter()
            .filter(|line| report_line_matches_profile(profile, line))
            .collect(),
    }
}

pub(super) fn execute_review_routine(
    state: &mut ServerState,
    catalog: &WorkflowCatalog,
    routine: &ReviewRoutineSpec,
    inputs: &[WorkflowInputAssignment],
) -> Result<ReviewRoutineExecutionResult, JsonRpcError> {
    if let Some(message) = routine.validation_error() {
        return Err(invalid_request(message));
    }
    if let Some(message) = validate_review_routine_input_assignments(routine, inputs) {
        return Err(invalid_request(message));
    }

    let intended_review_id = routine
        .save_review
        .enabled
        .then(|| intended_routine_review_id(routine, inputs))
        .transpose()?;
    if let Some(review_id) = intended_review_id.as_deref()
        && !routine.save_review.overwrite
    {
        reject_existing_review_run(state, review_id)?;
    }

    let source = match &routine.source {
        ReviewRoutineSource::Audit { audit, limit } => {
            let result = execute_corpus_audit_query(
                state,
                &CorpusAuditParams {
                    audit: *audit,
                    limit: *limit,
                },
            )?;
            ReviewRoutineSourceExecutionResult::Audit {
                result: Box::new(result),
            }
        }
        ReviewRoutineSource::Workflow { workflow_id } => {
            let workflow = catalog
                .workflow(workflow_id)
                .ok_or_else(|| invalid_request(format!("unknown workflow: {workflow_id}")))?;
            let result = execute_workflow_spec(state, &workflow, inputs)?;
            ReviewRoutineSourceExecutionResult::Workflow {
                result: Box::new(result),
            }
        }
        ReviewRoutineSource::Unsupported => {
            return Err(invalid_request(
                "review routine source kind is unsupported".to_owned(),
            ));
        }
    };

    let review_run = if let Some(review_id) = intended_review_id {
        Some(review_from_routine_source_result(
            routine, inputs, &source, review_id,
        )?)
    } else {
        None
    };

    let compare_diff = if routine.compare.is_some() {
        let review_run = review_run.as_ref().ok_or_else(|| {
            invalid_request(
                "review routine compare policy requires save_review to be enabled".to_owned(),
            )
        })?;
        latest_compatible_review_run(state, review_run)?
            .map(|base| ReviewRunDiff::between(&base, review_run).map(Box::new))
            .transpose()
            .map_err(invalid_request)?
    } else {
        None
    };

    let compare = routine.compare.as_ref().map(|policy| {
        let report = policy
            .report_profile_id
            .as_deref()
            .and_then(|profile_id| catalog.report_profile(profile_id))
            .map(|profile| {
                apply_report_profile(
                    &profile,
                    routine,
                    &source,
                    review_run.as_ref(),
                    compare_diff.as_deref(),
                )
            });
        ReviewRoutineCompareResult {
            target: policy.target,
            base_review: compare_diff.as_ref().map(|diff| diff.base_review.clone()),
            diff: compare_diff.clone(),
            report,
        }
    });

    let saved_review = if let Some(review_run) = &review_run {
        Some(save_review_run_with_policy(
            state,
            review_run,
            routine.save_review.overwrite,
        )?)
    } else {
        None
    };

    let reports = routine
        .report_profile_ids
        .iter()
        .map(|profile_id| {
            catalog
                .report_profile(profile_id)
                .ok_or_else(|| invalid_request(format!("unknown report profile: {profile_id}")))
                .map(|profile| {
                    apply_report_profile(
                        &profile,
                        routine,
                        &source,
                        review_run.as_ref(),
                        compare_diff.as_deref(),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ReviewRoutineExecutionResult {
        routine: routine.into(),
        source,
        saved_review,
        compare,
        reports,
    })
}

#[derive(Debug, Clone)]
enum WorkflowStepState {
    Resolve {
        node: Box<NodeRecord>,
    },
    Explore {
        focus_node_key: String,
        lens: ExplorationLens,
        limit: usize,
        unique: bool,
        result: Box<ExploreResult>,
    },
    Compare {
        left_node: Box<NodeRecord>,
        right_node: Box<NodeRecord>,
        group: NoteComparisonGroup,
        limit: usize,
        result: Box<NoteComparisonResult>,
    },
    ArtifactRun {
        artifact: Box<ExecutedExplorationArtifact>,
    },
    ArtifactSave {
        artifact: Box<ExplorationArtifactSummary>,
    },
}

impl WorkflowStepState {
    fn report(&self, step_id: String) -> WorkflowStepReport {
        let payload = match self {
            Self::Resolve { node } => WorkflowStepReportPayload::Resolve { node: node.clone() },
            Self::Explore {
                focus_node_key,
                result,
                ..
            } => WorkflowStepReportPayload::Explore {
                focus_node_key: focus_node_key.clone(),
                result: result.clone(),
            },
            Self::Compare {
                left_node,
                right_node,
                result,
                ..
            } => WorkflowStepReportPayload::Compare {
                left_node: left_node.clone(),
                right_node: right_node.clone(),
                result: result.clone(),
            },
            Self::ArtifactRun { artifact } => WorkflowStepReportPayload::ArtifactRun {
                artifact: artifact.clone(),
            },
            Self::ArtifactSave { artifact } => WorkflowStepReportPayload::ArtifactSave {
                artifact: artifact.clone(),
            },
        };
        WorkflowStepReport { step_id, payload }
    }
}

pub(super) fn execute_workflow_spec(
    state: &mut ServerState,
    workflow: &WorkflowSpec,
    inputs: &[WorkflowInputAssignment],
) -> Result<WorkflowExecutionResult, JsonRpcError> {
    if let Some(message) = workflow.validation_error() {
        return Err(invalid_request(message));
    }
    if let Some(message) = workflow.input_assignments_validation_error(inputs) {
        return Err(invalid_request(message));
    }

    let declared_input_kinds: HashMap<&str, slipbox_core::WorkflowInputKind> = workflow
        .inputs
        .iter()
        .map(|input| (input.input_id.as_str(), input.kind))
        .collect();
    let input_targets: HashMap<String, WorkflowResolveTarget> = inputs
        .iter()
        .map(|input| (input.input_id.clone(), input.target.clone()))
        .collect();
    let mut steps: HashMap<String, WorkflowStepState> =
        HashMap::with_capacity(workflow.steps.len());
    let mut reports = Vec::with_capacity(workflow.steps.len());

    for step in &workflow.steps {
        let step_state = (|| -> Result<WorkflowStepState, JsonRpcError> {
            match &step.payload {
                WorkflowStepPayload::Resolve { target } => {
                    let node = match target {
                        WorkflowResolveTarget::Input { input_id } => {
                            let target = input_targets.get(input_id).ok_or_else(|| {
                                invalid_request(format!(
                                    "workflow input {input_id} must be assigned"
                                ))
                            })?;
                            match declared_input_kinds.get(input_id.as_str()) {
                                Some(slipbox_core::WorkflowInputKind::NoteTarget) => {
                                    resolve_workflow_note_target(
                                        state,
                                        target,
                                        "workflow note target",
                                    )?
                                }
                                Some(slipbox_core::WorkflowInputKind::FocusTarget) => {
                                    resolve_workflow_note_target_from_focus(
                                        state,
                                        target,
                                        "workflow focus target",
                                    )?
                                }
                                None => {
                                    return Err(invalid_request(format!(
                                        "workflow input {input_id} must be declared"
                                    )));
                                }
                            }
                        }
                        _ => resolve_workflow_note_target(
                            state,
                            target,
                            "workflow note target",
                        )?,
                    };
                    Ok(WorkflowStepState::Resolve {
                        node: Box::new(node),
                    })
                }
                WorkflowStepPayload::Explore {
                    focus,
                    lens,
                    limit,
                    unique,
                } => {
                    let focus_node_key = match focus {
                        slipbox_core::WorkflowExploreFocus::NodeKey { node_key } => {
                            node_key.clone()
                        }
                        slipbox_core::WorkflowExploreFocus::Input { input_id } => {
                            if declared_input_kinds.get(input_id.as_str())
                                != Some(&slipbox_core::WorkflowInputKind::FocusTarget)
                            {
                                return Err(invalid_request(format!(
                                    "workflow input {input_id} must be declared as a focus-target input"
                                )));
                            }
                            let target = input_targets.get(input_id).ok_or_else(|| {
                                invalid_request(format!(
                                    "workflow input {input_id} must be assigned"
                                ))
                            })?;
                            resolve_workflow_focus_target(
                                state,
                                target,
                                *lens,
                                "workflow focus target",
                            )?
                        }
                        slipbox_core::WorkflowExploreFocus::ResolvedStep { step_id } => {
                            match steps.get(step_id) {
                                Some(WorkflowStepState::Resolve { node }) => node.node_key.clone(),
                                Some(other) => {
                                    return Err(invalid_request(format!(
                                        "expected resolve focus source, got {}",
                                        other.report(step_id.clone()).kind().label()
                                    )));
                                }
                                None => {
                                    return Err(invalid_request(format!(
                                        "references unknown focus step {}",
                                        step_id
                                    )));
                                }
                            }
                        }
                    };
                    let result = execute_explore_query(
                        state,
                        &ExploreParams {
                            node_key: focus_node_key.clone(),
                            lens: *lens,
                            limit: *limit,
                            unique: *unique,
                        },
                    )?;
                    Ok(WorkflowStepState::Explore {
                        focus_node_key,
                        lens: *lens,
                        limit: *limit,
                        unique: *unique,
                        result: Box::new(result),
                    })
                }
                WorkflowStepPayload::Compare {
                    left,
                    right,
                    group,
                    limit,
                } => {
                    let left_node = match steps.get(&left.step_id) {
                        Some(WorkflowStepState::Resolve { node }) => node.clone(),
                        _ => {
                            return Err(invalid_request(format!(
                                "references invalid left resolve step {}",
                                left.step_id
                            )));
                        }
                    };
                    let right_node = match steps.get(&right.step_id) {
                        Some(WorkflowStepState::Resolve { node }) => node.clone(),
                        _ => {
                            return Err(invalid_request(format!(
                                "references invalid right resolve step {}",
                                right.step_id
                            )));
                        }
                    };
                    let result = execute_compare_notes_query(
                        state,
                        &CompareNotesParams {
                            left_node_key: left_node.node_key.clone(),
                            right_node_key: right_node.node_key.clone(),
                            limit: *limit,
                        },
                    )?;
                    Ok(WorkflowStepState::Compare {
                        left_node,
                        right_node,
                        group: *group,
                        limit: *limit,
                        result: Box::new(result),
                    })
                }
                WorkflowStepPayload::ArtifactRun { artifact_id } => {
                    let artifact = execute_saved_exploration_artifact_by_id(state, artifact_id)?
                        .ok_or_else(|| {
                            invalid_request(format!("unknown exploration artifact: {artifact_id}"))
                        })?;
                    Ok(WorkflowStepState::ArtifactRun {
                        artifact: Box::new(artifact),
                    })
                }
                WorkflowStepPayload::ArtifactSave {
                    source,
                    metadata,
                    overwrite,
                } => {
                    let artifact = match source {
                        slipbox_core::WorkflowArtifactSaveSource::ExploreStep { step_id } => {
                            match steps.get(step_id) {
                                Some(WorkflowStepState::Explore {
                                    focus_node_key,
                                    lens,
                                    limit,
                                    unique,
                                    ..
                                }) => SavedExplorationArtifact {
                                    metadata: metadata.clone(),
                                    payload: ExplorationArtifactPayload::LensView {
                                        artifact: Box::new(SavedLensViewArtifact {
                                            root_node_key: focus_node_key.clone(),
                                            current_node_key: focus_node_key.clone(),
                                            lens: *lens,
                                            limit: *limit,
                                            unique: *unique,
                                            frozen_context: false,
                                        }),
                                    },
                                },
                                _ => {
                                    return Err(invalid_request(format!(
                                        "references invalid explore source {}",
                                        step_id
                                    )));
                                }
                            }
                        }
                        slipbox_core::WorkflowArtifactSaveSource::CompareStep { step_id } => {
                            match steps.get(step_id) {
                                Some(WorkflowStepState::Compare {
                                    left_node,
                                    right_node,
                                    group,
                                    limit,
                                    ..
                                }) => SavedExplorationArtifact {
                                    metadata: metadata.clone(),
                                    payload: ExplorationArtifactPayload::Comparison {
                                        artifact: Box::new(SavedComparisonArtifact {
                                            root_node_key: left_node.node_key.clone(),
                                            left_node_key: left_node.node_key.clone(),
                                            right_node_key: right_node.node_key.clone(),
                                            active_lens: ExplorationLens::Structure,
                                            structure_unique: false,
                                            comparison_group: *group,
                                            limit: *limit,
                                            frozen_context: false,
                                        }),
                                    },
                                },
                                _ => {
                                    return Err(invalid_request(format!(
                                        "references invalid compare source {}",
                                        step_id
                                    )));
                                }
                            }
                        }
                    };
                    let artifact =
                        save_exploration_artifact_with_policy(state, &artifact, *overwrite)?;
                    Ok(WorkflowStepState::ArtifactSave {
                        artifact: Box::new(artifact),
                    })
                }
            }
        })()
        .map_err(|error| with_step_context(&step.step_id, error))?;

        reports.push(step_state.report(step.step_id.clone()));
        steps.insert(step.step_id.clone(), step_state);
    }

    Ok(WorkflowExecutionResult {
        workflow: workflow.into(),
        steps: reports,
    })
}
fn known_workbench_pack(
    state: &ServerState,
    pack_id: &str,
) -> Result<WorkbenchPackManifest, JsonRpcError> {
    let pack = state
        .database
        .workbench_pack(pack_id)
        .map_err(|error| internal_error(error.context("failed to load workbench pack")))?;
    pack.ok_or_else(|| invalid_request(format!("unknown workbench pack: {pack_id}")))
}

#[derive(Debug, Deserialize)]
struct WorkbenchPackCompatibilityParams {
    pack: WorkbenchPackCompatibilityEnvelope,
}

fn workbench_pack_compatibility_issue(params: &serde_json::Value) -> Option<WorkbenchPackIssue> {
    let params = serde_json::from_value::<WorkbenchPackCompatibilityParams>(params.clone()).ok()?;
    params
        .pack
        .compatibility
        .validation_error()
        .map(|message| WorkbenchPackIssue {
            kind: WorkbenchPackIssueKind::UnsupportedVersion,
            asset_id: params.pack.pack_id,
            message,
        })
}
pub(crate) fn list_workflows(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let _params: ListWorkflowsParams = parse_params(params)?;
    let catalog = discover_server_workflow_catalog(state)?;
    to_value(ListWorkflowsResult {
        workflows: catalog.summaries(),
        issues: catalog.issues().to_vec(),
    })
}

pub(crate) fn workflow(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: WorkflowIdParams = parse_params(params)?;
    validate_workflow_id_params(&params)?;
    let workflow = discover_server_workflow_catalog(state)?
        .workflow(&params.workflow_id)
        .ok_or_else(|| invalid_request(format!("unknown workflow: {}", params.workflow_id)))?;
    to_value(WorkflowResult { workflow })
}

pub(crate) fn run_workflow(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RunWorkflowParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let workflow = discover_server_workflow_catalog(state)?
        .workflow(&params.workflow_id)
        .ok_or_else(|| invalid_request(format!("unknown workflow: {}", params.workflow_id)))?;
    let result = execute_workflow_spec(state, &workflow, &params.inputs)?;
    to_value(RunWorkflowResult { result })
}

pub(crate) fn list_review_routines(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let _params: ListReviewRoutinesParams = parse_params(params)?;
    let catalog = discover_server_workflow_catalog(state)?;
    to_value(ListReviewRoutinesResult {
        routines: catalog.review_routine_summaries(),
        issues: catalog.issues().to_vec(),
    })
}

pub(crate) fn review_routine(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewRoutineIdParams = parse_params(params)?;
    validate_review_routine_id_params(&params)?;
    let routine = discover_server_workflow_catalog(state)?
        .review_routine(&params.routine_id)
        .ok_or_else(|| invalid_request(format!("unknown review routine: {}", params.routine_id)))?;
    to_value(ReviewRoutineResult { routine })
}

pub(crate) fn run_review_routine(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RunReviewRoutineParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let catalog = discover_server_workflow_catalog(state)?;
    let routine = catalog
        .review_routine(&params.routine_id)
        .ok_or_else(|| invalid_request(format!("unknown review routine: {}", params.routine_id)))?;
    let result = execute_review_routine(state, &catalog, &routine, &params.inputs)?;
    to_value(RunReviewRoutineResult { result })
}
pub(crate) fn save_workflow_review(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SaveWorkflowReviewParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let review_id = intended_workflow_review_id(&params)?;
    let workflow = discover_server_workflow_catalog(state)?
        .workflow(&params.workflow_id)
        .ok_or_else(|| invalid_request(format!("unknown workflow: {}", params.workflow_id)))?;
    if !params.overwrite {
        reject_existing_review_run(state, &review_id)?;
    }
    let result = execute_workflow_spec(state, &workflow, &params.inputs)?;
    let review_run = review_from_workflow_result(&params, &result, review_id)?;
    let review = save_review_run_with_policy(state, &review_run, params.overwrite)?;
    to_value(SaveWorkflowReviewResult { result, review })
}
pub(crate) fn import_workbench_pack(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ImportWorkbenchPackParams = parse_params(params.clone()).map_err(|error| {
        workbench_pack_compatibility_issue(&params)
            .map(|issue| invalid_request(issue.message))
            .unwrap_or(error)
    })?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let pack = save_workbench_pack_with_policy(state, &params.pack, params.overwrite)?;
    to_value(ImportWorkbenchPackResult { pack })
}

pub(crate) fn workbench_pack(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: WorkbenchPackIdParams = parse_params(params)?;
    validate_pack_id_params(&params)?;
    to_value(WorkbenchPackResult {
        pack: known_workbench_pack(state, &params.pack_id)?,
    })
}

pub(crate) fn validate_workbench_pack(
    _state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ValidateWorkbenchPackParams = match parse_params(params.clone()) {
        Ok(params) => params,
        Err(error) => {
            if let Some(issue) = workbench_pack_compatibility_issue(&params) {
                return to_value(ValidateWorkbenchPackResult {
                    pack: None,
                    valid: false,
                    issues: vec![issue],
                });
            }
            return Err(error);
        }
    };
    let issues = params.pack.validation_issues();
    to_value(ValidateWorkbenchPackResult {
        pack: Some(params.pack.summary()),
        valid: issues.is_empty(),
        issues,
    })
}

pub(crate) fn export_workbench_pack(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: WorkbenchPackIdParams = parse_params(params)?;
    validate_pack_id_params(&params)?;
    to_value(known_workbench_pack(state, &params.pack_id)?)
}

pub(crate) fn list_workbench_packs(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let _params: ListWorkbenchPacksParams = parse_params(params)?;
    let packs = state
        .database
        .list_workbench_packs()
        .map_err(|error| internal_error(error.context("failed to list workbench packs")))?;
    let catalog = discover_workflow_catalog(&state.root, &state.workflow_dirs, &packs);
    to_value(ListWorkbenchPacksResult {
        packs: packs.iter().map(WorkbenchPackSummary::from).collect(),
        issues: catalog.issues().to_vec(),
    })
}

pub(crate) fn delete_workbench_pack(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: WorkbenchPackIdParams = parse_params(params)?;
    validate_pack_id_params(&params)?;
    if !state
        .database
        .delete_workbench_pack(&params.pack_id)
        .map_err(|error| internal_error(error.context("failed to delete workbench pack")))?
    {
        return Err(invalid_request(format!(
            "unknown workbench pack: {}",
            params.pack_id
        )));
    }
    to_value(DeleteWorkbenchPackResult {
        pack_id: params.pack_id,
    })
}
