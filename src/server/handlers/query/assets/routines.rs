use slipbox_core::{
    AppliedReportProfile, CorpusAuditParams, ListReviewRoutinesParams, ListReviewRoutinesResult,
    ReportProfileMode, ReportProfileSpec, ReviewFindingStatus, ReviewRoutineCompareResult,
    ReviewRoutineExecutionResult, ReviewRoutineIdParams, ReviewRoutineReportLine,
    ReviewRoutineResult, ReviewRoutineSource, ReviewRoutineSourceExecutionResult,
    ReviewRoutineSpec, ReviewRun, ReviewRunDiff, ReviewRunDiffBucket, ReviewRunSummary,
    RunReviewRoutineParams, RunReviewRoutineResult, SaveCorpusAuditReviewParams,
    SaveWorkflowReviewParams, WorkflowInputAssignment,
};
use slipbox_rpc::JsonRpcError;

use super::super::common::{invalid_request, validate_review_routine_id_params};
use super::super::reviews::{
    execute_corpus_audit_query, review_from_audit_result, save_review_run_with_policy,
};
use super::common::{
    discover_server_workflow_catalog, reject_existing_review_run, stable_json_fingerprint,
};
use super::workflows::{execute_workflow_spec, review_from_workflow_result};
use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;
use crate::server::workflows::WorkflowCatalog;

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

fn execute_review_routine(
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
