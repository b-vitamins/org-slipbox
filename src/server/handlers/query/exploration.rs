use slipbox_core::{
    CompareNotesParams, DeleteExplorationArtifactResult, ExecuteExplorationArtifactResult,
    ExecutedExplorationArtifact, ExecutedExplorationArtifactPayload, ExplorationArtifactIdParams,
    ExplorationArtifactResult, ExplorationArtifactSummary, ExplorationEntry, ExplorationLens,
    ExplorationSection, ExplorationSectionKind, ExploreParams, ExploreResult,
    ListExplorationArtifactsParams, ListExplorationArtifactsResult, NodeRecord,
    NoteComparisonResult, SaveExplorationArtifactParams, SaveExplorationArtifactResult,
    SavedComparisonArtifact, SavedExplorationArtifact, SavedLensViewArtifact, SavedTrailStep,
    TrailReplayResult, TrailReplayStepResult,
};
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

use super::common::{invalid_request, validate_artifact_id_params};
use crate::reflinks_query::query_reflinks;
use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;
use crate::unlinked_references_query::query_unlinked_references;

pub(super) fn execute_explore_query(
    state: &mut ServerState,
    params: &ExploreParams,
) -> Result<ExploreResult, JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(JsonRpcError::new(JsonRpcErrorObject::invalid_request(
            message,
        )));
    }

    let sections = match params.lens {
        ExplorationLens::Structure => {
            let node = state.known_note(&params.node_key, "explore query note")?;
            let backlinks = state
                .database
                .backlinks(
                    node.node_key.as_str(),
                    params.normalized_limit(),
                    params.unique,
                )
                .map_err(|error| internal_error(error.context("failed to query backlinks")))?;
            let forward_links = state
                .database
                .forward_links(
                    node.node_key.as_str(),
                    params.normalized_limit(),
                    params.unique,
                )
                .map_err(|error| internal_error(error.context("failed to query forward links")))?;
            vec![
                ExplorationSection {
                    kind: ExplorationSectionKind::Backlinks,
                    entries: backlinks
                        .into_iter()
                        .map(|record| ExplorationEntry::Backlink {
                            record: Box::new(record),
                        })
                        .collect(),
                },
                ExplorationSection {
                    kind: ExplorationSectionKind::ForwardLinks,
                    entries: forward_links
                        .into_iter()
                        .map(|record| ExplorationEntry::ForwardLink {
                            record: Box::new(record),
                        })
                        .collect(),
                },
            ]
        }
        ExplorationLens::Refs => {
            let anchor = state.known_anchor(&params.node_key, "explore query anchor")?;
            let reflinks = query_reflinks(
                &state.database,
                &state.root,
                &anchor,
                params.normalized_limit(),
            )
            .map_err(|error| internal_error(error.context("failed to query reflinks")))?;
            let unlinked_references = query_unlinked_references(
                &state.database,
                &state.root,
                &anchor,
                params.normalized_limit(),
            )
            .map_err(|error| {
                internal_error(error.context("failed to query unlinked references"))
            })?;
            vec![
                ExplorationSection {
                    kind: ExplorationSectionKind::Reflinks,
                    entries: reflinks
                        .into_iter()
                        .map(|record| ExplorationEntry::Reflink {
                            record: Box::new(record),
                        })
                        .collect(),
                },
                ExplorationSection {
                    kind: ExplorationSectionKind::UnlinkedReferences,
                    entries: unlinked_references
                        .into_iter()
                        .map(|record| ExplorationEntry::UnlinkedReference {
                            record: Box::new(record),
                        })
                        .collect(),
                },
            ]
        }
        ExplorationLens::Time => {
            let anchor = state.known_anchor(&params.node_key, "explore query anchor")?;
            let time_neighbors = state
                .database
                .time_neighbors(&anchor, params.normalized_limit())
                .map_err(|error| internal_error(error.context("failed to query time neighbors")))?;
            vec![ExplorationSection {
                kind: ExplorationSectionKind::TimeNeighbors,
                entries: time_neighbors
                    .into_iter()
                    .map(|record| ExplorationEntry::Anchor {
                        record: Box::new(record),
                    })
                    .collect(),
            }]
        }
        ExplorationLens::Tasks => {
            let anchor = state.known_anchor(&params.node_key, "explore query anchor")?;
            let task_neighbors = state
                .database
                .task_neighbors(&anchor, params.normalized_limit())
                .map_err(|error| internal_error(error.context("failed to query task neighbors")))?;
            vec![ExplorationSection {
                kind: ExplorationSectionKind::TaskNeighbors,
                entries: task_neighbors
                    .into_iter()
                    .map(|record| ExplorationEntry::Anchor {
                        record: Box::new(record),
                    })
                    .collect(),
            }]
        }
        ExplorationLens::Bridges => {
            let note = state.known_note(&params.node_key, "explore query note")?;
            let bridge_candidates = state
                .database
                .bridge_candidates(&note, params.normalized_limit())
                .map_err(|error| {
                    internal_error(error.context("failed to query bridge candidates"))
                })?;
            vec![ExplorationSection {
                kind: ExplorationSectionKind::BridgeCandidates,
                entries: bridge_candidates
                    .into_iter()
                    .map(|record| ExplorationEntry::Anchor {
                        record: Box::new(record),
                    })
                    .collect(),
            }]
        }
        ExplorationLens::Dormant => {
            let note = state.known_note(&params.node_key, "explore query note")?;
            let dormant_notes = state
                .database
                .dormant_related(&note, params.normalized_limit())
                .map_err(|error| internal_error(error.context("failed to query dormant notes")))?;
            vec![ExplorationSection {
                kind: ExplorationSectionKind::DormantNotes,
                entries: dormant_notes
                    .into_iter()
                    .map(|record| ExplorationEntry::Anchor {
                        record: Box::new(record),
                    })
                    .collect(),
            }]
        }
        ExplorationLens::Unresolved => {
            let note = state.known_note(&params.node_key, "explore query note")?;
            let unresolved_tasks = state
                .database
                .unresolved_tasks(&note, params.normalized_limit())
                .map_err(|error| {
                    internal_error(error.context("failed to query unresolved tasks"))
                })?;
            let weakly_integrated = state
                .database
                .weakly_integrated_notes(&note, params.normalized_limit())
                .map_err(|error| {
                    internal_error(error.context("failed to query weakly integrated notes"))
                })?;
            vec![
                ExplorationSection {
                    kind: ExplorationSectionKind::UnresolvedTasks,
                    entries: unresolved_tasks
                        .into_iter()
                        .map(|record| ExplorationEntry::Anchor {
                            record: Box::new(record),
                        })
                        .collect(),
                },
                ExplorationSection {
                    kind: ExplorationSectionKind::WeaklyIntegratedNotes,
                    entries: weakly_integrated
                        .into_iter()
                        .map(|record| ExplorationEntry::Anchor {
                            record: Box::new(record),
                        })
                        .collect(),
                },
            ]
        }
    };

    Ok(ExploreResult {
        lens: params.lens,
        sections,
    })
}

fn execute_saved_lens_view(
    state: &mut ServerState,
    artifact: &SavedLensViewArtifact,
) -> Result<(NodeRecord, NodeRecord, ExploreResult), JsonRpcError> {
    let root_note = state
        .known_note_for_node_or_anchor(&artifact.root_node_key, "saved lens-view root focus")?;
    let current_note = state.known_note_for_node_or_anchor(
        &artifact.current_node_key,
        "saved lens-view current focus",
    )?;
    let result = execute_explore_query(state, &artifact.explore_params())?;
    Ok((root_note, current_note, result))
}

pub(super) fn execute_compare_notes_query(
    state: &mut ServerState,
    params: &CompareNotesParams,
) -> Result<NoteComparisonResult, JsonRpcError> {
    let left = state.known_note(&params.left_node_key, "left comparison note")?;
    let right = state.known_note(&params.right_node_key, "right comparison note")?;
    state
        .database
        .compare_notes(&left, &right, params)
        .map_err(|error| internal_error(error.context("failed to compare notes")))
}

fn execute_saved_comparison(
    state: &mut ServerState,
    artifact: &SavedComparisonArtifact,
) -> Result<(NodeRecord, NoteComparisonResult), JsonRpcError> {
    let root_note = state.known_note(&artifact.root_node_key, "saved comparison root note")?;
    let result = execute_compare_notes_query(state, &artifact.compare_notes_params())?;
    Ok((root_note, result))
}

fn replay_saved_trail_step(
    state: &mut ServerState,
    step: &SavedTrailStep,
) -> Result<TrailReplayStepResult, JsonRpcError> {
    match step {
        SavedTrailStep::LensView { artifact } => {
            let (root_note, current_note, result) = execute_saved_lens_view(state, artifact)?;
            Ok(TrailReplayStepResult::LensView {
                artifact: artifact.clone(),
                root_note: Box::new(root_note),
                current_note: Box::new(current_note),
                result: Box::new(result),
            })
        }
        SavedTrailStep::Comparison { artifact } => {
            let (root_note, result) = execute_saved_comparison(state, artifact)?;
            Ok(TrailReplayStepResult::Comparison {
                artifact: artifact.clone(),
                root_note: Box::new(root_note),
                result: Box::new(result),
            })
        }
    }
}

pub(super) fn execute_saved_exploration_artifact(
    state: &mut ServerState,
    artifact: &SavedExplorationArtifact,
) -> Result<ExecutedExplorationArtifact, JsonRpcError> {
    if let Some(message) = artifact.validation_error() {
        return Err(JsonRpcError::new(JsonRpcErrorObject::invalid_request(
            message,
        )));
    }

    let payload = match &artifact.payload {
        slipbox_core::ExplorationArtifactPayload::LensView { artifact } => {
            let (root_note, current_note, result) = execute_saved_lens_view(state, artifact)?;
            ExecutedExplorationArtifactPayload::LensView {
                artifact: artifact.clone(),
                root_note: Box::new(root_note),
                current_note: Box::new(current_note),
                result: Box::new(result),
            }
        }
        slipbox_core::ExplorationArtifactPayload::Comparison { artifact } => {
            let (root_note, result) = execute_saved_comparison(state, artifact)?;
            ExecutedExplorationArtifactPayload::Comparison {
                artifact: artifact.clone(),
                root_note: Box::new(root_note),
                result: Box::new(result),
            }
        }
        slipbox_core::ExplorationArtifactPayload::Trail { artifact } => {
            let steps = artifact
                .steps
                .iter()
                .map(|step| replay_saved_trail_step(state, step))
                .collect::<Result<Vec<_>, _>>()?;
            let detached_step = artifact
                .detached_step
                .as_deref()
                .map(|step| replay_saved_trail_step(state, step).map(Box::new))
                .transpose()?;
            ExecutedExplorationArtifactPayload::Trail {
                artifact: artifact.clone(),
                replay: Box::new(TrailReplayResult {
                    steps,
                    cursor: artifact.cursor,
                    detached_step,
                }),
            }
        }
    };

    Ok(ExecutedExplorationArtifact {
        metadata: artifact.metadata.clone(),
        payload,
    })
}

pub(super) fn execute_saved_exploration_artifact_by_id(
    state: &mut ServerState,
    artifact_id: &str,
) -> Result<Option<ExecutedExplorationArtifact>, JsonRpcError> {
    let artifact = state
        .database
        .exploration_artifact(artifact_id)
        .map_err(|error| internal_error(error.context("failed to load exploration artifact")))?;
    artifact
        .as_ref()
        .map(|saved| execute_saved_exploration_artifact(state, saved))
        .transpose()
}
pub(super) fn save_exploration_artifact_with_policy(
    state: &mut ServerState,
    artifact: &SavedExplorationArtifact,
    overwrite: bool,
) -> Result<ExplorationArtifactSummary, JsonRpcError> {
    if overwrite {
        state
            .database
            .save_exploration_artifact(artifact)
            .map_err(|error| {
                internal_error(error.context("failed to save exploration artifact"))
            })?;
    } else if !state
        .database
        .save_exploration_artifact_if_absent(artifact)
        .map_err(|error| {
            internal_error(error.context("failed to save exploration artifact without overwrite"))
        })?
    {
        return Err(invalid_request(format!(
            "exploration artifact already exists: {}",
            artifact.metadata.artifact_id
        )));
    }

    Ok(ExplorationArtifactSummary::from(artifact))
}
fn known_exploration_artifact(
    state: &ServerState,
    artifact_id: &str,
) -> Result<SavedExplorationArtifact, JsonRpcError> {
    let artifact = state
        .database
        .exploration_artifact(artifact_id)
        .map_err(|error| internal_error(error.context("failed to load exploration artifact")))?;
    artifact.ok_or_else(|| invalid_request(format!("unknown exploration artifact: {artifact_id}")))
}
pub(crate) fn save_exploration_artifact(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SaveExplorationArtifactParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let artifact =
        save_exploration_artifact_with_policy(state, &params.artifact, params.overwrite)?;
    to_value(SaveExplorationArtifactResult { artifact })
}

pub(crate) fn exploration_artifact(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ExplorationArtifactIdParams = parse_params(params)?;
    validate_artifact_id_params(&params)?;
    to_value(ExplorationArtifactResult {
        artifact: known_exploration_artifact(state, &params.artifact_id)?,
    })
}

pub(crate) fn list_exploration_artifacts(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let _params: ListExplorationArtifactsParams = parse_params(params)?;
    let artifacts = state
        .database
        .list_exploration_artifacts()
        .map_err(|error| internal_error(error.context("failed to list exploration artifacts")))?;
    to_value(ListExplorationArtifactsResult {
        artifacts: artifacts
            .iter()
            .map(ExplorationArtifactSummary::from)
            .collect(),
    })
}

pub(crate) fn delete_exploration_artifact(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ExplorationArtifactIdParams = parse_params(params)?;
    validate_artifact_id_params(&params)?;
    if !state
        .database
        .delete_exploration_artifact(&params.artifact_id)
        .map_err(|error| internal_error(error.context("failed to delete exploration artifact")))?
    {
        return Err(invalid_request(format!(
            "unknown exploration artifact: {}",
            params.artifact_id
        )));
    }
    to_value(DeleteExplorationArtifactResult {
        artifact_id: params.artifact_id,
    })
}

pub(crate) fn execute_exploration_artifact(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ExplorationArtifactIdParams = parse_params(params)?;
    validate_artifact_id_params(&params)?;
    let artifact = execute_saved_exploration_artifact_by_id(state, &params.artifact_id)?
        .ok_or_else(|| {
            invalid_request(format!(
                "unknown exploration artifact: {}",
                params.artifact_id
            ))
        })?;
    to_value(ExecuteExplorationArtifactResult { artifact })
}
pub(crate) fn explore(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ExploreParams = parse_params(params)?;
    to_value(execute_explore_query(state, &params)?)
}
pub(crate) fn compare_notes(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: CompareNotesParams = parse_params(params)?;
    to_value(execute_compare_notes_query(state, &params)?)
}
