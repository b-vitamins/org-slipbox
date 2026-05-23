use std::collections::HashMap;

use slipbox_core::{
    CompareNotesParams, ExecutedExplorationArtifact, ExplorationArtifactSummary, ExplorationLens,
    ExploreParams, ExploreResult, ListWorkflowsParams, ListWorkflowsResult, NodeRecord,
    NoteComparisonGroup, NoteComparisonResult, ReviewFinding, ReviewFindingPayload,
    ReviewFindingStatus, ReviewRun, ReviewRunMetadata, ReviewRunPayload, RunWorkflowParams,
    RunWorkflowResult, SaveWorkflowReviewParams, SaveWorkflowReviewResult,
    SavedExplorationArtifact, WorkflowArtifactSaveSource, WorkflowExecutionResult,
    WorkflowExploreFocus, WorkflowIdParams, WorkflowInputAssignment, WorkflowInputKind,
    WorkflowResolveTarget, WorkflowResult, WorkflowSpec, WorkflowStepPayload, WorkflowStepReport,
    WorkflowStepReportPayload,
};
use slipbox_rpc::JsonRpcError;

use super::super::common::{invalid_request, validate_workflow_id_params, with_step_context};
use super::super::exploration::{
    execute_compare_notes_query, execute_explore_query, execute_saved_exploration_artifact_by_id,
    save_exploration_artifact_with_policy,
};
use super::super::reviews::save_review_run_with_policy;
use super::common::{
    discover_server_workflow_catalog, reject_existing_review_run, resolve_workflow_focus_target,
    resolve_workflow_note_target, resolve_workflow_note_target_from_focus, stable_json_fingerprint,
};
use crate::server::rpc::{parse_params, to_value};
use crate::server::state::ServerState;

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

pub(super) fn review_from_workflow_result(
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

pub(in crate::server::handlers::query) fn execute_workflow_spec(
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

    let declared_input_kinds: HashMap<&str, WorkflowInputKind> = workflow
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
                                Some(WorkflowInputKind::NoteTarget) => {
                                    resolve_workflow_note_target(
                                        state,
                                        target,
                                        "workflow note target",
                                    )?
                                }
                                Some(WorkflowInputKind::FocusTarget) => {
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
                        WorkflowExploreFocus::NodeKey { node_key } => node_key.clone(),
                        WorkflowExploreFocus::Input { input_id } => {
                            if declared_input_kinds.get(input_id.as_str())
                                != Some(&WorkflowInputKind::FocusTarget)
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
                        WorkflowExploreFocus::ResolvedStep { step_id } => match steps.get(step_id) {
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
                        },
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
                        WorkflowArtifactSaveSource::ExploreStep { step_id } => {
                            match steps.get(step_id) {
                                Some(WorkflowStepState::Explore {
                                    focus_node_key,
                                    lens,
                                    limit,
                                    unique,
                                    ..
                                }) => SavedExplorationArtifact::live_lens_view(
                                    metadata.clone(),
                                    focus_node_key.clone(),
                                    *lens,
                                    *limit,
                                    *unique,
                                ),
                                _ => {
                                    return Err(invalid_request(format!(
                                        "references invalid explore source {}",
                                        step_id
                                    )));
                                }
                            }
                        }
                        WorkflowArtifactSaveSource::CompareStep { step_id } => {
                            match steps.get(step_id) {
                                Some(WorkflowStepState::Compare {
                                    left_node,
                                    right_node,
                                    group,
                                    limit,
                                    ..
                                }) => SavedExplorationArtifact::live_comparison(
                                    metadata.clone(),
                                    left_node.node_key.clone(),
                                    right_node.node_key.clone(),
                                    *group,
                                    *limit,
                                ),
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
