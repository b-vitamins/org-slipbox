use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slipbox_core::{
    AgendaParams, AgendaResult, AppliedReportProfile, BacklinksParams, BacklinksResult,
    CompareNotesParams, CorpusAuditEntry, CorpusAuditKind, CorpusAuditParams, CorpusAuditResult,
    DeleteExplorationArtifactResult, DeleteReviewRunResult, DeleteWorkbenchPackResult,
    ExecuteExplorationArtifactResult, ExecutedExplorationArtifact,
    ExecutedExplorationArtifactPayload, ExplorationArtifactIdParams, ExplorationArtifactPayload,
    ExplorationArtifactResult, ExplorationArtifactSummary, ExplorationEntry, ExplorationLens,
    ExplorationSection, ExplorationSectionKind, ExploreParams, ExploreResult, ForwardLinksParams,
    ForwardLinksResult, GraphParams, GraphResult, ImportWorkbenchPackParams,
    ImportWorkbenchPackResult, IndexFileParams, IndexFileResult, IndexedFilesResult,
    ListExplorationArtifactsParams, ListExplorationArtifactsResult, ListReviewRoutinesParams,
    ListReviewRoutinesResult, ListReviewRunsParams, ListReviewRunsResult, ListWorkbenchPacksParams,
    ListWorkbenchPacksResult, ListWorkflowsParams, ListWorkflowsResult, MarkReviewFindingParams,
    MarkReviewFindingResult, NodeAtPointParams, NodeFromIdParams, NodeFromKeyParams,
    NodeFromRefParams, NodeFromTitleOrAliasParams, NodeRecord, NoteComparisonGroup,
    NoteComparisonResult, PingInfo, RandomNodeResult, ReflinksParams, ReflinksResult,
    ReportProfileMode, ReportProfileSpec, ReviewFinding, ReviewFindingPayload,
    ReviewFindingRemediationPreview, ReviewFindingRemediationPreviewParams,
    ReviewFindingRemediationPreviewResult, ReviewFindingStatus, ReviewFindingStatusTransition,
    ReviewRoutineCompareResult, ReviewRoutineExecutionResult, ReviewRoutineIdParams,
    ReviewRoutineReportLine, ReviewRoutineResult, ReviewRoutineSource,
    ReviewRoutineSourceExecutionResult, ReviewRoutineSpec, ReviewRun, ReviewRunDiff,
    ReviewRunDiffBucket, ReviewRunDiffParams, ReviewRunDiffResult, ReviewRunIdParams,
    ReviewRunMetadata, ReviewRunPayload, ReviewRunResult, ReviewRunSummary, RunReviewRoutineParams,
    RunReviewRoutineResult, RunWorkflowParams, RunWorkflowResult, SaveCorpusAuditReviewParams,
    SaveCorpusAuditReviewResult, SaveExplorationArtifactParams, SaveExplorationArtifactResult,
    SaveReviewRunParams, SaveReviewRunResult, SaveWorkflowReviewParams, SaveWorkflowReviewResult,
    SavedComparisonArtifact, SavedExplorationArtifact, SavedLensViewArtifact, SavedTrailStep,
    SearchFilesParams, SearchFilesResult, SearchNodesParams, SearchNodesResult,
    SearchOccurrencesParams, SearchOccurrencesResult, SearchRefsParams, SearchRefsResult,
    SearchTagsParams, SearchTagsResult, StatusInfo, TrailReplayResult, TrailReplayStepResult,
    UnlinkedReferencesParams, UnlinkedReferencesResult, ValidateWorkbenchPackParams,
    ValidateWorkbenchPackResult, WorkbenchPackCompatibilityEnvelope, WorkbenchPackIdParams,
    WorkbenchPackIssue, WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackResult,
    WorkbenchPackSummary, WorkflowExecutionResult, WorkflowIdParams, WorkflowInputAssignment,
    WorkflowResolveTarget, WorkflowResult, WorkflowSpec, WorkflowStepPayload, WorkflowStepReport,
    WorkflowStepReportPayload,
};
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

use crate::occurrences_query::query_occurrences;
use crate::reflinks_query::query_reflinks;
use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;
use crate::server::workflows::{WorkflowCatalog, discover_workflow_catalog};
use crate::unlinked_references_query::query_unlinked_references;

pub(crate) fn ping(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    to_value(PingInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        root: state.root.display().to_string(),
        db: state.db_path.display().to_string(),
    })
}

pub(crate) fn status(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let stats = state
        .database
        .stats()
        .map_err(|error| internal_error(error.context("failed to read index statistics")))?;
    to_value(StatusInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        root: state.root.display().to_string(),
        db: state.db_path.display().to_string(),
        files_indexed: stats.files_indexed,
        nodes_indexed: stats.nodes_indexed,
        links_indexed: stats.links_indexed,
    })
}

pub(crate) fn index(state: &mut ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let files = slipbox_index::scan_root_with_policy(&state.root, &state.discovery)
        .map_err(|error| internal_error(error.context("failed to scan Org files")))?;
    let stats = state
        .database
        .sync_index(&files)
        .map_err(|error| internal_error(error.context("failed to update SQLite index")))?;
    to_value(stats)
}

pub(crate) fn graph_dot(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let mut params: GraphParams = parse_params(params)?;
    let hidden_link_types = params.normalized_hidden_link_types();
    if let Some(unsupported) = hidden_link_types
        .iter()
        .find(|link_type| link_type.as_str() != "id")
    {
        return Err(JsonRpcError::new(JsonRpcErrorObject::invalid_request(
            format!("unsupported graph link type filter: {unsupported}"),
        )));
    }
    params.hidden_link_types = hidden_link_types;

    if let Some(root_node_key) = params.root_node_key.as_deref() {
        state.known_note(root_node_key, "graph root node")?;
    }

    let dot = state
        .database
        .graph_dot(&params)
        .map_err(|error| internal_error(error.context("failed to generate graph DOT")))?;
    to_value(GraphResult { dot })
}

pub(crate) fn search_nodes(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchNodesParams = parse_params(params)?;
    let nodes = state
        .database
        .search_nodes(
            &params.query,
            params.normalized_limit(),
            params.sort.clone(),
        )
        .map_err(|error| internal_error(error.context("failed to query nodes")))?;
    to_value(SearchNodesResult { nodes })
}

pub(crate) fn random_node(
    state: &mut ServerState,
    _params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let node = state
        .database
        .random_node()
        .map_err(|error| internal_error(error.context("failed to query random node")))?;
    to_value(RandomNodeResult { node })
}

pub(crate) fn indexed_files(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let files = state
        .database
        .indexed_files()
        .map_err(|error| internal_error(error.context("failed to read indexed files")))?;
    to_value(IndexedFilesResult { files })
}

pub(crate) fn search_files(
    state: &ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchFilesParams = parse_params(params)?;
    let files = state
        .database
        .search_files(&params.query, params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to query indexed files")))?;
    to_value(SearchFilesResult { files })
}

pub(crate) fn search_occurrences(
    state: &ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchOccurrencesParams = parse_params(params)?;
    let occurrences = query_occurrences(&state.database, &params.query, params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to query text occurrences")))?;
    to_value(SearchOccurrencesResult { occurrences })
}

pub(crate) fn search_tags(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchTagsParams = parse_params(params)?;
    let tags = state
        .database
        .search_tags(&params.query, params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to query tags")))?;
    to_value(SearchTagsResult { tags })
}

pub(crate) fn node_from_id(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeFromIdParams = parse_params(params)?;
    let node = state
        .database
        .node_from_id(&params.id)
        .map_err(|error| internal_error(error.context("failed to resolve node ID")))?;
    to_value(node)
}

pub(crate) fn node_from_key(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeFromKeyParams = parse_params(params)?;
    let node = state
        .database
        .note_by_key(&params.node_key)
        .map_err(|error| internal_error(error.context("failed to resolve node key")))?;
    to_value(node)
}

pub(crate) fn node_from_title_or_alias(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeFromTitleOrAliasParams = parse_params(params)?;
    let matches = state
        .database
        .node_from_title_or_alias(&params.title_or_alias, params.nocase)
        .map_err(|error| internal_error(error.context("failed to resolve node title or alias")))?;
    if matches.len() > 1 {
        return Err(JsonRpcError::new(JsonRpcErrorObject::invalid_request(
            format!("multiple nodes match {}", params.title_or_alias),
        )));
    }
    to_value(matches.into_iter().next())
}

pub(crate) fn node_at_point(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeAtPointParams = parse_params(params)?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let node = state
        .database
        .node_at_point(&relative_path, params.normalized_line())
        .map_err(|error| internal_error(error.context("failed to resolve node at point")))?;
    to_value(node)
}

pub(crate) fn anchor_at_point(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeAtPointParams = parse_params(params)?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let anchor = state
        .database
        .anchor_at_point(&relative_path, params.normalized_line())
        .map_err(|error| internal_error(error.context("failed to resolve anchor at point")))?;
    to_value(anchor)
}

pub(crate) fn backlinks(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: BacklinksParams = parse_params(params)?;
    let backlinks = state
        .database
        .backlinks(&params.node_key, params.normalized_limit(), params.unique)
        .map_err(|error| internal_error(error.context("failed to query backlinks")))?;
    to_value(BacklinksResult { backlinks })
}

pub(crate) fn forward_links(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ForwardLinksParams = parse_params(params)?;
    let forward_links = state
        .database
        .forward_links(&params.node_key, params.normalized_limit(), params.unique)
        .map_err(|error| internal_error(error.context("failed to query forward links")))?;
    to_value(ForwardLinksResult { forward_links })
}

pub(crate) fn reflinks(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReflinksParams = parse_params(params)?;
    let node = state.known_anchor(&params.node_key, "reflink query anchor")?;
    let reflinks = query_reflinks(
        &state.database,
        &state.root,
        &node,
        params.normalized_limit(),
    )
    .map_err(|error| internal_error(error.context("failed to query reflinks")))?;
    to_value(ReflinksResult { reflinks })
}

pub(crate) fn unlinked_references(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: UnlinkedReferencesParams = parse_params(params)?;
    let node = state.known_anchor(&params.node_key, "unlinked-reference query anchor")?;
    let unlinked_references = query_unlinked_references(
        &state.database,
        &state.root,
        &node,
        params.normalized_limit(),
    )
    .map_err(|error| internal_error(error.context("failed to query unlinked references")))?;
    to_value(UnlinkedReferencesResult {
        unlinked_references,
    })
}

fn execute_explore_query(
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

fn execute_compare_notes_query(
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

pub(crate) fn execute_saved_exploration_artifact(
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

pub(crate) fn execute_saved_exploration_artifact_by_id(
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

fn invalid_request(message: String) -> JsonRpcError {
    JsonRpcError::new(JsonRpcErrorObject::invalid_request(message))
}

fn with_step_context(step_id: &str, error: JsonRpcError) -> JsonRpcError {
    let inner = error.into_inner();
    JsonRpcError::new(JsonRpcErrorObject {
        code: inner.code,
        message: format!("workflow step {step_id} failed: {}", inner.message),
    })
}

fn validate_artifact_id_params(params: &ExplorationArtifactIdParams) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}

fn validate_review_id_params(params: &ReviewRunIdParams) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}

fn validate_pack_id_params(params: &WorkbenchPackIdParams) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}

fn validate_workflow_id_params(params: &WorkflowIdParams) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}

fn validate_review_routine_id_params(params: &ReviewRoutineIdParams) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
}

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

fn save_exploration_artifact_with_policy(
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

fn save_review_run_with_policy(
    state: &mut ServerState,
    review: &ReviewRun,
    overwrite: bool,
) -> Result<ReviewRunSummary, JsonRpcError> {
    if overwrite {
        state
            .database
            .save_review_run(review)
            .map_err(|error| internal_error(error.context("failed to save review run")))?;
    } else if !state
        .database
        .save_review_run_if_absent(review)
        .map_err(|error| {
            internal_error(error.context("failed to save review run without overwrite"))
        })?
    {
        return Err(invalid_request(format!(
            "review run already exists: {}",
            review.metadata.review_id
        )));
    }

    Ok(ReviewRunSummary::from(review))
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

fn render_audit_kind(kind: CorpusAuditKind) -> &'static str {
    match kind {
        CorpusAuditKind::DanglingLinks => "dangling-links",
        CorpusAuditKind::DuplicateTitles => "duplicate-titles",
        CorpusAuditKind::OrphanNotes => "orphan-notes",
        CorpusAuditKind::WeaklyIntegratedNotes => "weakly-integrated-notes",
    }
}

fn title_for_audit_kind(kind: CorpusAuditKind) -> &'static str {
    match kind {
        CorpusAuditKind::DanglingLinks => "Dangling Links",
        CorpusAuditKind::DuplicateTitles => "Duplicate Titles",
        CorpusAuditKind::OrphanNotes => "Orphan Notes",
        CorpusAuditKind::WeaklyIntegratedNotes => "Weakly Integrated Notes",
    }
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

fn generated_audit_review_id(params: &CorpusAuditParams) -> String {
    format!(
        "review/audit/{}/limit-{}",
        render_audit_kind(params.audit),
        params.normalized_limit()
    )
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

fn audit_finding_id(entry: &CorpusAuditEntry) -> String {
    match entry {
        CorpusAuditEntry::DanglingLink { record } => format!(
            "audit/dangling-links/{}/{}/{}/{}",
            record.source.node_key, record.missing_explicit_id, record.line, record.column
        ),
        CorpusAuditEntry::DuplicateTitle { record } => {
            let mut node_keys = record
                .notes
                .iter()
                .map(|note| note.node_key.as_str())
                .collect::<Vec<_>>();
            node_keys.sort_unstable();
            format!("audit/duplicate-titles/{}", node_keys.join(","))
        }
        CorpusAuditEntry::OrphanNote { record } => {
            format!("audit/orphan-notes/{}", record.note.node_key)
        }
        CorpusAuditEntry::WeaklyIntegratedNote { record } => {
            format!("audit/weakly-integrated-notes/{}", record.note.node_key)
        }
    }
}

fn review_from_audit_result(
    params: &SaveCorpusAuditReviewParams,
    result: &CorpusAuditResult,
) -> Result<ReviewRun, JsonRpcError> {
    let audit_params = params.audit_params();
    let metadata = ReviewRunMetadata {
        review_id: params
            .review_id
            .clone()
            .unwrap_or_else(|| generated_audit_review_id(&audit_params)),
        title: params
            .title
            .clone()
            .unwrap_or_else(|| format!("{} Review", title_for_audit_kind(result.audit))),
        summary: params.summary.clone().or_else(|| {
            Some(format!(
                "{} findings from {} audit with limit {}",
                result.entries.len(),
                render_audit_kind(result.audit),
                audit_params.normalized_limit()
            ))
        }),
    };
    let review = ReviewRun {
        metadata,
        payload: ReviewRunPayload::Audit {
            audit: result.audit,
            limit: audit_params.normalized_limit(),
        },
        findings: result
            .entries
            .iter()
            .map(|entry| ReviewFinding {
                finding_id: audit_finding_id(entry),
                status: ReviewFindingStatus::Open,
                payload: ReviewFindingPayload::Audit {
                    entry: Box::new(entry.clone()),
                },
            })
            .collect(),
    };
    if let Some(message) = review.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(review)
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

fn execute_workflow_spec(
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

fn known_review_run(state: &ServerState, review_id: &str) -> Result<ReviewRun, JsonRpcError> {
    let review = state
        .database
        .review_run(review_id)
        .map_err(|error| internal_error(error.context("failed to load review run")))?;
    review.ok_or_else(|| invalid_request(format!("unknown review run: {review_id}")))
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

pub(crate) fn save_review_run(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SaveReviewRunParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let review = save_review_run_with_policy(state, &params.review, params.overwrite)?;
    to_value(SaveReviewRunResult { review })
}

pub(crate) fn review_run(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewRunIdParams = parse_params(params)?;
    validate_review_id_params(&params)?;
    to_value(ReviewRunResult {
        review: known_review_run(state, &params.review_id)?,
    })
}

pub(crate) fn diff_review_runs(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewRunDiffParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let base = known_review_run(state, &params.base_review_id)?;
    let target = known_review_run(state, &params.target_review_id)?;
    let diff = ReviewRunDiff::between(&base, &target).map_err(invalid_request)?;
    to_value(ReviewRunDiffResult { diff })
}

pub(crate) fn review_finding_remediation_preview(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewFindingRemediationPreviewParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let review = known_review_run(state, &params.review_id)?;
    let finding = review
        .findings
        .iter()
        .find(|finding| finding.finding_id == params.finding_id)
        .ok_or_else(|| {
            invalid_request(format!(
                "unknown review finding {} in review run {}",
                params.finding_id, params.review_id
            ))
        })?;
    let preview = ReviewFindingRemediationPreview::from_review_finding(&params.review_id, finding)
        .map_err(invalid_request)?;
    to_value(ReviewFindingRemediationPreviewResult { preview })
}

pub(crate) fn list_review_runs(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let _params: ListReviewRunsParams = parse_params(params)?;
    let reviews = state
        .database
        .list_review_runs()
        .map_err(|error| internal_error(error.context("failed to list review runs")))?;
    to_value(ListReviewRunsResult {
        reviews: reviews.iter().map(ReviewRunSummary::from).collect(),
    })
}

pub(crate) fn delete_review_run(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReviewRunIdParams = parse_params(params)?;
    validate_review_id_params(&params)?;
    if !state
        .database
        .delete_review_run(&params.review_id)
        .map_err(|error| internal_error(error.context("failed to delete review run")))?
    {
        return Err(invalid_request(format!(
            "unknown review run: {}",
            params.review_id
        )));
    }
    to_value(DeleteReviewRunResult {
        review_id: params.review_id,
    })
}

pub(crate) fn mark_review_finding(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: MarkReviewFindingParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let mut review = known_review_run(state, &params.review_id)?;
    let finding = review
        .findings
        .iter_mut()
        .find(|finding| finding.finding_id == params.finding_id)
        .ok_or_else(|| {
            invalid_request(format!(
                "unknown review finding {} in review run {}",
                params.finding_id, params.review_id
            ))
        })?;
    let from_status = finding.status;
    let transition = ReviewFindingStatusTransition {
        review_id: params.review_id.clone(),
        finding_id: params.finding_id.clone(),
        from_status,
        to_status: params.status,
    };
    if let Some(message) = transition.validation_error() {
        return Err(invalid_request(message));
    }
    finding.status = params.status;
    state
        .database
        .save_review_run(&review)
        .map_err(|error| internal_error(error.context("failed to save review run")))?;
    to_value(MarkReviewFindingResult { transition })
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

pub(crate) fn corpus_audit(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: CorpusAuditParams = parse_params(params)?;
    to_value(execute_corpus_audit_query(state, &params)?)
}

fn execute_corpus_audit_query(
    state: &mut ServerState,
    params: &CorpusAuditParams,
) -> Result<CorpusAuditResult, JsonRpcError> {
    let entries = match params.audit {
        CorpusAuditKind::DanglingLinks => state
            .database
            .audit_dangling_links(params.normalized_limit())
            .map_err(|error| internal_error(error.context("failed to query dangling link audit")))?
            .into_iter()
            .map(|record| CorpusAuditEntry::DanglingLink {
                record: Box::new(record),
            })
            .collect(),
        CorpusAuditKind::DuplicateTitles => state
            .database
            .audit_duplicate_titles(params.normalized_limit())
            .map_err(|error| {
                internal_error(error.context("failed to query duplicate title audit"))
            })?
            .into_iter()
            .map(|record| CorpusAuditEntry::DuplicateTitle {
                record: Box::new(record),
            })
            .collect(),
        CorpusAuditKind::OrphanNotes => state
            .database
            .audit_orphan_notes(params.normalized_limit())
            .map_err(|error| internal_error(error.context("failed to query orphan note audit")))?
            .into_iter()
            .map(|record| CorpusAuditEntry::OrphanNote {
                record: Box::new(record),
            })
            .collect(),
        CorpusAuditKind::WeaklyIntegratedNotes => state
            .database
            .audit_weakly_integrated_notes(params.normalized_limit())
            .map_err(|error| {
                internal_error(error.context("failed to query weakly integrated note audit"))
            })?
            .into_iter()
            .map(|record| CorpusAuditEntry::WeaklyIntegratedNote {
                record: Box::new(record),
            })
            .collect(),
    };
    Ok(CorpusAuditResult {
        audit: params.audit,
        entries,
    })
}

pub(crate) fn save_corpus_audit_review(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SaveCorpusAuditReviewParams = parse_params(params)?;
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    let result = execute_corpus_audit_query(state, &params.audit_params())?;
    let review_run = review_from_audit_result(&params, &result)?;
    let review = save_review_run_with_policy(state, &review_run, params.overwrite)?;
    to_value(SaveCorpusAuditReviewResult { result, review })
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

pub(crate) fn explore(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ExploreParams = parse_params(params)?;
    to_value(execute_explore_query(state, &params)?)
}

pub(crate) fn search_refs(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchRefsParams = parse_params(params)?;
    let refs = state
        .database
        .search_refs(&params.query, params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to query refs")))?;
    to_value(SearchRefsResult { refs })
}

pub(crate) fn node_from_ref(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeFromRefParams = parse_params(params)?;
    let node = state
        .database
        .node_from_ref(&params.reference)
        .map_err(|error| internal_error(error.context("failed to resolve ref")))?;
    to_value(node)
}

pub(crate) fn agenda(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AgendaParams = parse_params(params)?;
    let nodes = state
        .database
        .agenda_nodes(&params.start, &params.end, params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to query agenda")))?;
    to_value(AgendaResult { nodes })
}

pub(crate) fn compare_notes(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: CompareNotesParams = parse_params(params)?;
    to_value(execute_compare_notes_query(state, &params)?)
}

pub(crate) fn index_file(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: IndexFileParams = parse_params(params)?;
    let (relative_path, absolute_path) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    if absolute_path.exists() && state.discovery.matches_path(&state.root, &absolute_path) {
        state.sync_path(&absolute_path)?;
    } else {
        state
            .database
            .remove_file_index(&relative_path)
            .map_err(|error| {
                internal_error(error.context("failed to remove file from SQLite index"))
            })?;
    }
    to_value(IndexFileResult {
        file_path: relative_path,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    use serde_json::json;
    use slipbox_core::{
        AnchorRecord, AuditRemediationConfidence, AuditRemediationPreviewPayload,
        BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID, BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
        BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID, BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
        BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID, CompareNotesParams,
        ComparisonConnectorDirection, CorpusAuditEntry, CorpusAuditKind, CorpusAuditResult,
        DanglingLinkAuditRecord, DeleteExplorationArtifactResult, DeleteReviewRunResult,
        DeleteWorkbenchPackResult, ExecuteExplorationArtifactResult,
        ExecutedExplorationArtifactPayload, ExplorationArtifactMetadata,
        ExplorationArtifactPayload, ExplorationArtifactResult, ExplorationEntry,
        ExplorationExplanation, ExplorationLens, ExplorationSectionKind, ExploreParams,
        ExploreResult, ImportWorkbenchPackResult, ListExplorationArtifactsResult,
        ListReviewRunsResult, ListWorkbenchPacksResult, ListWorkflowsResult,
        MarkReviewFindingResult, NodeKind, NoteComparisonEntry, NoteComparisonExplanation,
        NoteComparisonGroup, NoteComparisonResult, NoteComparisonSectionKind, ReportJsonlLineKind,
        ReportProfileMetadata, ReportProfileMode, ReportProfileSpec, ReportProfileSubject,
        ReviewFinding, ReviewFindingPayload, ReviewFindingRemediationPreviewResult,
        ReviewFindingStatus, ReviewRoutineComparePolicy, ReviewRoutineCompareTarget,
        ReviewRoutineMetadata, ReviewRoutineReportLine, ReviewRoutineSaveReviewPolicy,
        ReviewRoutineSource, ReviewRoutineSourceExecutionResult, ReviewRoutineSpec, ReviewRun,
        ReviewRunDiffBucket, ReviewRunDiffResult, ReviewRunMetadata, ReviewRunPayload,
        ReviewRunResult, RunReviewRoutineResult, RunWorkflowResult, SaveCorpusAuditReviewResult,
        SaveExplorationArtifactResult, SaveReviewRunResult, SaveWorkflowReviewResult,
        SavedComparisonArtifact, SavedExplorationArtifact, SavedLensViewArtifact,
        SavedTrailArtifact, SavedTrailStep, TrailReplayStepResult, ValidateWorkbenchPackResult,
        WorkbenchPackCompatibility, WorkbenchPackIssueKind, WorkbenchPackManifest,
        WorkbenchPackMetadata, WorkbenchPackResult, WorkflowInputAssignment, WorkflowMetadata,
        WorkflowResolveTarget, WorkflowResult, WorkflowSpec, WorkflowSpecCompatibility,
        WorkflowStepPayload, WorkflowStepReport, WorkflowStepReportPayload, WorkflowStepSpec,
    };
    use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};
    use tempfile::TempDir;

    use super::{
        compare_notes, corpus_audit, delete_exploration_artifact, delete_review_run,
        delete_workbench_pack, diff_review_runs, execute_compare_notes_query,
        execute_exploration_artifact, execute_explore_query, execute_saved_exploration_artifact,
        execute_saved_exploration_artifact_by_id, execute_workflow_spec, exploration_artifact,
        explore, export_workbench_pack, import_workbench_pack, list_exploration_artifacts,
        list_review_runs, list_workbench_packs, list_workflows, mark_review_finding,
        review_finding_remediation_preview, review_run, run_review_routine, run_workflow,
        save_corpus_audit_review, save_exploration_artifact, save_review_run, save_workflow_review,
        validate_workbench_pack, workbench_pack, workflow,
    };
    use crate::server::state::ServerState;

    #[test]
    fn explore_dispatches_declared_lenses() {
        let (_workspace, mut state, target_key) = indexed_state();

        let structure: ExploreResult = serde_json::from_value(
            explore(
                &mut state,
                json!({
                    "node_key": target_key.as_str(),
                    "lens": "structure",
                    "limit": 20
                }),
            )
            .expect("structure lens should succeed"),
        )
        .expect("structure result should deserialize");
        assert_eq!(
            structure
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![
                ExplorationSectionKind::Backlinks,
                ExplorationSectionKind::ForwardLinks
            ]
        );
        assert!(!structure.sections[0].entries.is_empty());

        let refs: ExploreResult = serde_json::from_value(
            explore(
                &mut state,
                json!({
                    "node_key": target_key.as_str(),
                    "lens": "refs",
                    "limit": 20
                }),
            )
            .expect("refs lens should succeed"),
        )
        .expect("refs result should deserialize");
        assert_eq!(
            refs.sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![
                ExplorationSectionKind::Reflinks,
                ExplorationSectionKind::UnlinkedReferences
            ]
        );
        assert!(
            refs.sections[0]
                .entries
                .iter()
                .any(|entry| matches!(entry, ExplorationEntry::Reflink { .. }))
        );
        assert!(
            refs.sections[1]
                .entries
                .iter()
                .any(|entry| matches!(entry, ExplorationEntry::UnlinkedReference { .. }))
        );

        let time: ExploreResult = serde_json::from_value(
            explore(
                &mut state,
                json!({
                    "node_key": target_key.as_str(),
                    "lens": "time",
                    "limit": 20
                }),
            )
            .expect("time lens should succeed"),
        )
        .expect("time result should deserialize");
        assert_eq!(time.sections.len(), 1);
        assert_eq!(time.sections[0].kind, ExplorationSectionKind::TimeNeighbors);
        assert!(
            time.sections[0]
                .entries
                .iter()
                .any(|entry| matches!(entry, ExplorationEntry::Anchor { .. }))
        );

        let tasks: ExploreResult = serde_json::from_value(
            explore(
                &mut state,
                json!({
                    "node_key": target_key.as_str(),
                    "lens": "tasks",
                    "limit": 20
                }),
            )
            .expect("tasks lens should succeed"),
        )
        .expect("tasks result should deserialize");
        assert_eq!(tasks.sections.len(), 1);
        assert_eq!(
            tasks.sections[0].kind,
            ExplorationSectionKind::TaskNeighbors
        );
        assert!(
            tasks.sections[0]
                .entries
                .iter()
                .any(|entry| matches!(entry, ExplorationEntry::Anchor { .. }))
        );
    }

    #[test]
    fn explore_rejects_unique_outside_structure() {
        let (_workspace, mut state, target_key) = indexed_state();

        let error = explore(
            &mut state,
            json!({
                "node_key": target_key.as_str(),
                "lens": "refs",
                "limit": 20,
                "unique": true
            }),
        )
        .expect_err("refs lens should reject unique");

        assert_eq!(
            error.into_inner().message,
            "explore unique is only supported for the structure lens"
        );
    }

    #[test]
    fn compare_notes_dispatches_structured_sections() {
        let (_workspace, mut state, left_key, right_key) = comparison_state();

        let comparison: NoteComparisonResult = serde_json::from_value(
            compare_notes(
                &mut state,
                json!({
                    "left_node_key": left_key.as_str(),
                    "right_node_key": right_key.as_str(),
                    "limit": 20
                }),
            )
            .expect("comparison should succeed"),
        )
        .expect("comparison result should deserialize");

        assert_eq!(
            comparison
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![
                NoteComparisonSectionKind::SharedRefs,
                NoteComparisonSectionKind::SharedPlanningDates,
                NoteComparisonSectionKind::LeftOnlyRefs,
                NoteComparisonSectionKind::RightOnlyRefs,
                NoteComparisonSectionKind::SharedBacklinks,
                NoteComparisonSectionKind::SharedForwardLinks,
                NoteComparisonSectionKind::ContrastingTaskStates,
                NoteComparisonSectionKind::PlanningTensions,
                NoteComparisonSectionKind::IndirectConnectors,
            ]
        );

        assert!(comparison.sections[0].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Reference { record }
            if record.reference == "@shared2024"
                && record.explanation == NoteComparisonExplanation::SharedReference
        )));
        assert!(comparison.sections[1].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::PlanningRelation { record }
            if record.date == "2026-05-01T00:00:00"
                && record.explanation == NoteComparisonExplanation::SharedPlanningDate
        )));
        assert!(comparison.sections[2].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Reference { record }
            if record.reference == "@left2024"
                && record.explanation == NoteComparisonExplanation::LeftOnlyReference
        )));
        assert!(comparison.sections[3].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Reference { record }
            if record.reference == "@right2024"
                && record.explanation == NoteComparisonExplanation::RightOnlyReference
        )));
        assert!(comparison.sections[4].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Node { record }
            if record.node.title == "Shared Backlink"
                && record.explanation == NoteComparisonExplanation::SharedBacklink
        )));
        assert!(comparison.sections[5].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Node { record }
            if record.node.title == "Shared Forward"
                && record.explanation == NoteComparisonExplanation::SharedForwardLink
        )));
        assert!(comparison.sections[6].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::TaskState { record }
            if record.left_todo_keyword == "TODO"
                && record.right_todo_keyword == "NEXT"
                && record.explanation == NoteComparisonExplanation::ContrastingTaskState
        )));
        assert!(comparison.sections[7].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::PlanningRelation { record }
            if record.date == "2026-05-01T00:00:00"
                && record.explanation == NoteComparisonExplanation::PlanningTension
        )));
        assert!(comparison.sections[8].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Node { record }
            if record.node.title == "Left To Right Bridge"
                && record.explanation == NoteComparisonExplanation::IndirectConnector {
                    direction: ComparisonConnectorDirection::LeftToRight,
                }
        )));
        assert!(comparison.sections[8].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Node { record }
            if record.node.title == "Right To Left Bridge"
                && record.explanation == NoteComparisonExplanation::IndirectConnector {
                    direction: ComparisonConnectorDirection::RightToLeft,
                }
        )));
    }

    #[test]
    fn explore_dispatches_non_obvious_lenses() {
        let (_workspace, mut state, focus_key) = non_obvious_state();

        let bridges: ExploreResult = serde_json::from_value(
            explore(
                &mut state,
                json!({
                    "node_key": focus_key.as_str(),
                    "lens": "bridges",
                    "limit": 20
                }),
            )
            .expect("bridges lens should succeed"),
        )
        .expect("bridges result should deserialize");
        assert_eq!(bridges.sections.len(), 1);
        assert_eq!(
            bridges.sections[0].kind,
            ExplorationSectionKind::BridgeCandidates
        );
        assert!(bridges.sections[0].entries.iter().any(|entry| matches!(
            entry,
            ExplorationEntry::Anchor { record }
            if record.anchor.title == "Dormant Bridge"
                && matches!(
                    record.explanation,
                    ExplorationExplanation::BridgeCandidate { ref references, ref via_notes }
                    if references == &vec!["@shared2024".to_owned()]
                        && via_notes.len() == 1
                        && via_notes[0].title == "Neighbor"
                        && via_notes[0].explicit_id.as_deref() == Some("neighbor-id")
                )
        )));

        let dormant: ExploreResult = serde_json::from_value(
            explore(
                &mut state,
                json!({
                    "node_key": focus_key.as_str(),
                    "lens": "dormant",
                    "limit": 20
                }),
            )
            .expect("dormant lens should succeed"),
        )
        .expect("dormant result should deserialize");
        assert_eq!(dormant.sections.len(), 1);
        assert_eq!(
            dormant.sections[0].kind,
            ExplorationSectionKind::DormantNotes
        );
        assert!(dormant.sections[0].entries.iter().any(|entry| matches!(
            entry,
            ExplorationEntry::Anchor { record }
            if record.anchor.title == "Dormant Bridge"
                && matches!(
                    record.explanation,
                    ExplorationExplanation::DormantSharedReference { ref references, .. }
                    if references == &vec!["@shared2024".to_owned()]
                )
        )));

        let unresolved: ExploreResult = serde_json::from_value(
            explore(
                &mut state,
                json!({
                    "node_key": focus_key.as_str(),
                    "lens": "unresolved",
                    "limit": 20
                }),
            )
            .expect("unresolved lens should succeed"),
        )
        .expect("unresolved result should deserialize");
        assert_eq!(
            unresolved
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![
                ExplorationSectionKind::UnresolvedTasks,
                ExplorationSectionKind::WeaklyIntegratedNotes,
            ]
        );
        assert!(unresolved.sections[0].entries.iter().any(|entry| matches!(
            entry,
            ExplorationEntry::Anchor { record }
            if record.anchor.title == "Unresolved Thread"
                && record.explanation == ExplorationExplanation::UnresolvedSharedReference {
                    references: vec!["@shared2024".to_owned()],
                    todo_keyword: "TODO".to_owned(),
                }
        )));
        assert!(unresolved.sections[1].entries.iter().any(|entry| matches!(
            entry,
            ExplorationEntry::Anchor { record }
            if record.anchor.title == "Weak Thread"
                && record.explanation
                    == ExplorationExplanation::WeaklyIntegratedSharedReference {
                        references: vec!["@shared2024".to_owned()],
                        structural_link_count: 0,
                    }
        )));
    }

    #[test]
    fn saved_lens_artifacts_execute_like_live_queries() {
        let (_workspace, mut state, target_key) = indexed_state();

        let cases = [
            (
                "saved-structure",
                saved_lens_artifact(
                    "saved-structure",
                    "Saved Structure",
                    &target_key,
                    ExplorationLens::Structure,
                ),
                ExploreParams {
                    node_key: target_key.clone(),
                    lens: ExplorationLens::Structure,
                    limit: 20,
                    unique: false,
                },
            ),
            (
                "saved-refs",
                saved_lens_artifact(
                    "saved-refs",
                    "Saved Refs",
                    &target_key,
                    ExplorationLens::Refs,
                ),
                ExploreParams {
                    node_key: target_key.clone(),
                    lens: ExplorationLens::Refs,
                    limit: 20,
                    unique: false,
                },
            ),
            (
                "saved-time",
                saved_lens_artifact(
                    "saved-time",
                    "Saved Time",
                    &target_key,
                    ExplorationLens::Time,
                ),
                ExploreParams {
                    node_key: target_key.clone(),
                    lens: ExplorationLens::Time,
                    limit: 20,
                    unique: false,
                },
            ),
            (
                "saved-tasks",
                saved_lens_artifact(
                    "saved-tasks",
                    "Saved Tasks",
                    &target_key,
                    ExplorationLens::Tasks,
                ),
                ExploreParams {
                    node_key: target_key.clone(),
                    lens: ExplorationLens::Tasks,
                    limit: 20,
                    unique: false,
                },
            ),
        ];

        for (artifact_id, artifact, params) in cases {
            state
                .database
                .save_exploration_artifact(&artifact)
                .expect("artifact should save");
            let live =
                execute_explore_query(&mut state, &params).expect("live explore should succeed");
            let executed = execute_saved_exploration_artifact_by_id(&mut state, artifact_id)
                .expect("saved artifact execution should succeed")
                .expect("saved artifact should exist");

            assert_eq!(executed.metadata, artifact.metadata);
            match executed.payload {
                ExecutedExplorationArtifactPayload::LensView {
                    artifact: executed_artifact,
                    result,
                    ..
                } => {
                    match artifact.payload {
                        ExplorationArtifactPayload::LensView { artifact } => {
                            assert_eq!(executed_artifact, artifact);
                        }
                        _ => panic!("expected saved lens-view artifact"),
                    }
                    assert_eq!(*result, live);
                }
                payload => panic!("expected lens-view execution, got {:?}", payload.kind()),
            }
        }
    }

    #[test]
    fn saved_non_obvious_lens_artifacts_execute_like_live_queries() {
        let (_workspace, mut state, focus_key) = non_obvious_state();

        let cases = [
            ("saved-bridges", ExplorationLens::Bridges),
            ("saved-dormant", ExplorationLens::Dormant),
            ("saved-unresolved", ExplorationLens::Unresolved),
        ];

        for (artifact_id, lens) in cases {
            let artifact = saved_lens_artifact(artifact_id, artifact_id, &focus_key, lens);
            state
                .database
                .save_exploration_artifact(&artifact)
                .expect("artifact should save");
            let live = execute_explore_query(
                &mut state,
                &ExploreParams {
                    node_key: focus_key.clone(),
                    lens,
                    limit: 20,
                    unique: false,
                },
            )
            .expect("live non-obvious explore should succeed");
            let executed = execute_saved_exploration_artifact_by_id(&mut state, artifact_id)
                .expect("saved artifact execution should succeed")
                .expect("saved artifact should exist");

            match executed.payload {
                ExecutedExplorationArtifactPayload::LensView { result, .. } => {
                    assert_eq!(*result, live);
                }
                payload => panic!("expected lens-view execution, got {:?}", payload.kind()),
            }
        }
    }

    #[test]
    fn saved_comparison_artifact_executes_like_live_queries() {
        let (_workspace, mut state, left_key, right_key) = comparison_state();
        let artifact = saved_comparison_artifact(
            "saved-comparison",
            "Saved Comparison",
            &left_key,
            &right_key,
        );
        state
            .database
            .save_exploration_artifact(&artifact)
            .expect("artifact should save");

        let live = execute_compare_notes_query(
            &mut state,
            &CompareNotesParams {
                left_node_key: left_key.clone(),
                right_node_key: right_key.clone(),
                limit: 20,
            },
        )
        .expect("live comparison should succeed");
        let executed = execute_saved_exploration_artifact_by_id(&mut state, "saved-comparison")
            .expect("saved comparison should execute")
            .expect("saved comparison should exist");

        assert_eq!(executed.metadata, artifact.metadata);
        match executed.payload {
            ExecutedExplorationArtifactPayload::Comparison {
                artifact: executed_artifact,
                result,
                ..
            } => {
                match artifact.payload {
                    ExplorationArtifactPayload::Comparison { artifact } => {
                        assert_eq!(executed_artifact, artifact);
                    }
                    _ => panic!("expected saved comparison artifact"),
                }
                assert_eq!(*result, live);
            }
            payload => panic!("expected comparison execution, got {:?}", payload.kind()),
        }
    }

    #[test]
    fn saved_trail_artifacts_replay_live_step_results() {
        let (_workspace, mut state, left_key, right_key) = comparison_state();
        let lens_step = SavedLensViewArtifact {
            root_node_key: left_key.clone(),
            current_node_key: left_key.clone(),
            lens: ExplorationLens::Structure,
            limit: 20,
            unique: false,
            frozen_context: false,
        };
        let comparison_step = SavedComparisonArtifact {
            root_node_key: left_key.clone(),
            left_node_key: left_key.clone(),
            right_node_key: right_key.clone(),
            active_lens: ExplorationLens::Structure,
            structure_unique: false,
            comparison_group: Default::default(),
            limit: 20,
            frozen_context: false,
        };
        let detached_step = SavedLensViewArtifact {
            root_node_key: right_key.clone(),
            current_node_key: right_key.clone(),
            lens: ExplorationLens::Structure,
            limit: 20,
            unique: false,
            frozen_context: false,
        };
        let artifact = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "saved-trail".to_owned(),
                title: "Saved Trail".to_owned(),
                summary: Some("Mixed trail replay".to_owned()),
            },
            payload: ExplorationArtifactPayload::Trail {
                artifact: Box::new(SavedTrailArtifact {
                    steps: vec![
                        SavedTrailStep::LensView {
                            artifact: Box::new(lens_step.clone()),
                        },
                        SavedTrailStep::Comparison {
                            artifact: Box::new(comparison_step.clone()),
                        },
                    ],
                    cursor: 1,
                    detached_step: Some(Box::new(SavedTrailStep::LensView {
                        artifact: Box::new(detached_step.clone()),
                    })),
                }),
            },
        };
        state
            .database
            .save_exploration_artifact(&artifact)
            .expect("trail artifact should save");

        let expected_lens = execute_explore_query(&mut state, &lens_step.explore_params())
            .expect("live lens replay should succeed");
        let expected_comparison =
            execute_compare_notes_query(&mut state, &comparison_step.compare_notes_params())
                .expect("live comparison replay should succeed");
        let expected_detached = execute_explore_query(&mut state, &detached_step.explore_params())
            .expect("live detached replay should succeed");

        let executed = execute_saved_exploration_artifact_by_id(&mut state, "saved-trail")
            .expect("saved trail should execute")
            .expect("saved trail should exist");

        assert_eq!(executed.metadata, artifact.metadata);
        match executed.payload {
            ExecutedExplorationArtifactPayload::Trail {
                artifact: executed_artifact,
                replay,
            } => {
                match artifact.payload {
                    ExplorationArtifactPayload::Trail { artifact } => {
                        assert_eq!(executed_artifact, artifact);
                    }
                    _ => panic!("expected saved trail artifact"),
                }
                assert_eq!(replay.cursor, 1);
                assert_eq!(replay.steps.len(), 2);
                match &replay.steps[0] {
                    TrailReplayStepResult::LensView {
                        artifact, result, ..
                    } => {
                        assert_eq!(artifact.as_ref(), &lens_step);
                        assert_eq!(result.as_ref(), &expected_lens);
                    }
                    other => panic!(
                        "expected first replay step to be lens-view, got {:?}",
                        other
                    ),
                }
                match &replay.steps[1] {
                    TrailReplayStepResult::Comparison {
                        artifact, result, ..
                    } => {
                        assert_eq!(artifact.as_ref(), &comparison_step);
                        assert_eq!(result.as_ref(), &expected_comparison);
                    }
                    other => {
                        panic!(
                            "expected second replay step to be comparison, got {:?}",
                            other
                        )
                    }
                }
                match replay.detached_step.as_deref() {
                    Some(TrailReplayStepResult::LensView {
                        artifact, result, ..
                    }) => {
                        assert_eq!(artifact.as_ref(), &detached_step);
                        assert_eq!(result.as_ref(), &expected_detached);
                    }
                    other => panic!("expected detached replay step, got {:?}", other),
                }
            }
            payload => panic!("expected trail execution, got {:?}", payload.kind()),
        }
    }

    #[test]
    fn saved_artifact_execution_returns_none_when_id_is_missing() {
        let (_workspace, mut state, target_key) = indexed_state();
        let _ = target_key;
        assert_eq!(
            execute_saved_exploration_artifact_by_id(&mut state, "missing-artifact")
                .expect("lookup should succeed"),
            None
        );
    }

    #[test]
    fn direct_saved_artifact_execution_rejects_invalid_artifacts() {
        let (_workspace, mut state, target_key) = indexed_state();
        let invalid = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "invalid-trail".to_owned(),
                title: "Invalid Trail".to_owned(),
                summary: None,
            },
            payload: ExplorationArtifactPayload::Trail {
                artifact: Box::new(SavedTrailArtifact {
                    steps: vec![SavedTrailStep::LensView {
                        artifact: Box::new(SavedLensViewArtifact {
                            root_node_key: target_key.clone(),
                            current_node_key: target_key,
                            lens: ExplorationLens::Structure,
                            limit: 20,
                            unique: false,
                            frozen_context: false,
                        }),
                    }],
                    cursor: 1,
                    detached_step: None,
                }),
            },
        };

        let error = execute_saved_exploration_artifact(&mut state, &invalid)
            .expect_err("direct execution should reject malformed artifacts");
        assert_eq!(
            error.into_inner().message,
            "trail cursor must point to an existing step"
        );
    }

    #[test]
    fn artifact_rpc_operations_round_trip_saved_artifacts() {
        let (_workspace, mut state, focus_key) = non_obvious_state();
        let artifact = saved_lens_artifact(
            "saved-unresolved",
            "Saved Unresolved",
            &focus_key,
            ExplorationLens::Unresolved,
        );

        let saved: SaveExplorationArtifactResult = serde_json::from_value(
            save_exploration_artifact(
                &mut state,
                json!({ "artifact": artifact.clone(), "overwrite": true }),
            )
            .expect("save artifact RPC should succeed"),
        )
        .expect("save result should decode");
        assert_eq!(saved.artifact.metadata, artifact.metadata);
        assert_eq!(saved.artifact.kind, artifact.kind());

        let listed: ListExplorationArtifactsResult = serde_json::from_value(
            list_exploration_artifacts(&mut state, json!({}))
                .expect("list artifacts RPC should succeed"),
        )
        .expect("list result should decode");
        assert_eq!(listed.artifacts.len(), 1);
        assert_eq!(listed.artifacts[0], saved.artifact);

        let inspected: ExplorationArtifactResult = serde_json::from_value(
            exploration_artifact(&mut state, json!({ "artifact_id": "saved-unresolved" }))
                .expect("inspect artifact RPC should succeed"),
        )
        .expect("inspect result should decode");
        assert_eq!(inspected.artifact, artifact);

        let live = execute_explore_query(
            &mut state,
            &ExploreParams {
                node_key: focus_key,
                lens: ExplorationLens::Unresolved,
                limit: 20,
                unique: false,
            },
        )
        .expect("live explore should succeed");
        let executed: ExecuteExplorationArtifactResult = serde_json::from_value(
            execute_exploration_artifact(&mut state, json!({ "artifact_id": "saved-unresolved" }))
                .expect("execute artifact RPC should succeed"),
        )
        .expect("execute result should decode");
        assert_eq!(executed.artifact.metadata, artifact.metadata);
        match executed.artifact.payload {
            ExecutedExplorationArtifactPayload::LensView {
                artifact: executed_artifact,
                result,
                ..
            } => {
                match artifact.payload {
                    ExplorationArtifactPayload::LensView { artifact } => {
                        assert_eq!(executed_artifact, artifact);
                    }
                    _ => panic!("expected saved lens-view artifact"),
                }
                assert_eq!(*result, live);
            }
            payload => panic!("expected lens-view execution, got {:?}", payload.kind()),
        }

        let deleted: DeleteExplorationArtifactResult = serde_json::from_value(
            delete_exploration_artifact(&mut state, json!({ "artifact_id": "saved-unresolved" }))
                .expect("delete artifact RPC should succeed"),
        )
        .expect("delete result should decode");
        assert_eq!(deleted.artifact_id, "saved-unresolved");

        let listed_after_delete: ListExplorationArtifactsResult = serde_json::from_value(
            list_exploration_artifacts(&mut state, json!({}))
                .expect("list after delete should succeed"),
        )
        .expect("list after delete should decode");
        assert!(listed_after_delete.artifacts.is_empty());
    }

    #[test]
    fn artifact_rpc_replays_saved_comparisons_and_trails_after_reopen() {
        let (_workspace, mut state, left_key, right_key) = comparison_state();
        let comparison = saved_comparison_artifact(
            "saved-comparison",
            "Saved Comparison",
            &left_key,
            &right_key,
        );
        let trail = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "saved-trail".to_owned(),
                title: "Saved Trail".to_owned(),
                summary: Some("Persisted replay".to_owned()),
            },
            payload: ExplorationArtifactPayload::Trail {
                artifact: Box::new(SavedTrailArtifact {
                    steps: vec![
                        SavedTrailStep::LensView {
                            artifact: Box::new(SavedLensViewArtifact {
                                root_node_key: left_key.clone(),
                                current_node_key: left_key.clone(),
                                lens: ExplorationLens::Structure,
                                limit: 20,
                                unique: false,
                                frozen_context: false,
                            }),
                        },
                        SavedTrailStep::Comparison {
                            artifact: Box::new(SavedComparisonArtifact {
                                root_node_key: left_key.clone(),
                                left_node_key: left_key.clone(),
                                right_node_key: right_key.clone(),
                                active_lens: ExplorationLens::Structure,
                                structure_unique: false,
                                comparison_group: Default::default(),
                                limit: 20,
                                frozen_context: false,
                            }),
                        },
                    ],
                    cursor: 1,
                    detached_step: Some(Box::new(SavedTrailStep::LensView {
                        artifact: Box::new(SavedLensViewArtifact {
                            root_node_key: right_key.clone(),
                            current_node_key: right_key.clone(),
                            lens: ExplorationLens::Structure,
                            limit: 20,
                            unique: false,
                            frozen_context: false,
                        }),
                    })),
                }),
            },
        };

        for artifact in [comparison.clone(), trail.clone()] {
            let _: SaveExplorationArtifactResult = serde_json::from_value(
                save_exploration_artifact(
                    &mut state,
                    json!({ "artifact": artifact, "overwrite": true }),
                )
                .expect("save artifact RPC should succeed"),
            )
            .expect("save result should decode");
        }

        let root = state.root.clone();
        let db_path = state.db_path.clone();
        let discovery = state.discovery.clone();
        drop(state);

        let mut reopened = ServerState::new(root, db_path, Vec::new(), discovery)
            .expect("state should reopen cleanly");

        let listed: ListExplorationArtifactsResult = serde_json::from_value(
            list_exploration_artifacts(&mut reopened, json!({}))
                .expect("list after reopen should succeed"),
        )
        .expect("list after reopen should decode");
        let mut ids = listed
            .artifacts
            .into_iter()
            .map(|summary| summary.metadata.artifact_id)
            .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(
            ids,
            vec!["saved-comparison".to_owned(), "saved-trail".to_owned()]
        );

        let expected_left_structure = execute_explore_query(
            &mut reopened,
            &ExploreParams {
                node_key: left_key.clone(),
                lens: ExplorationLens::Structure,
                limit: 20,
                unique: false,
            },
        )
        .expect("live left structure should succeed");
        let expected_right_structure = execute_explore_query(
            &mut reopened,
            &ExploreParams {
                node_key: right_key.clone(),
                lens: ExplorationLens::Structure,
                limit: 20,
                unique: false,
            },
        )
        .expect("live right structure should succeed");
        let expected_comparison = execute_compare_notes_query(
            &mut reopened,
            &CompareNotesParams {
                left_node_key: left_key.clone(),
                right_node_key: right_key.clone(),
                limit: 20,
            },
        )
        .expect("live comparison should succeed");

        let executed_comparison: ExecuteExplorationArtifactResult = serde_json::from_value(
            execute_exploration_artifact(
                &mut reopened,
                json!({ "artifact_id": "saved-comparison" }),
            )
            .expect("comparison execution after reopen should succeed"),
        )
        .expect("comparison execution result should decode");
        match executed_comparison.artifact.payload {
            ExecutedExplorationArtifactPayload::Comparison {
                artifact: executed_artifact,
                result,
                ..
            } => {
                match comparison.payload {
                    ExplorationArtifactPayload::Comparison { artifact } => {
                        assert_eq!(executed_artifact, artifact);
                    }
                    _ => panic!("expected saved comparison artifact"),
                }
                assert_eq!(*result, expected_comparison);
            }
            payload => panic!("expected comparison execution, got {:?}", payload.kind()),
        }

        let executed_trail: ExecuteExplorationArtifactResult = serde_json::from_value(
            execute_exploration_artifact(&mut reopened, json!({ "artifact_id": "saved-trail" }))
                .expect("trail execution after reopen should succeed"),
        )
        .expect("trail execution result should decode");
        match executed_trail.artifact.payload {
            ExecutedExplorationArtifactPayload::Trail {
                artifact: executed_artifact,
                replay,
            } => {
                match trail.payload {
                    ExplorationArtifactPayload::Trail { artifact } => {
                        assert_eq!(executed_artifact, artifact);
                    }
                    _ => panic!("expected saved trail artifact"),
                }
                assert_eq!(replay.cursor, 1);
                assert_eq!(replay.steps.len(), 2);
                match &replay.steps[0] {
                    TrailReplayStepResult::LensView {
                        artifact, result, ..
                    } => {
                        match &executed_artifact.steps[0] {
                            SavedTrailStep::LensView {
                                artifact: expected_artifact,
                            } => {
                                assert_eq!(artifact.as_ref(), expected_artifact.as_ref());
                            }
                            _ => panic!("expected first trail step artifact to be lens-view"),
                        }
                        assert_eq!(result.as_ref(), &expected_left_structure);
                    }
                    other => panic!(
                        "expected first replay step to be lens-view, got {:?}",
                        other
                    ),
                }
                match &replay.steps[1] {
                    TrailReplayStepResult::Comparison {
                        artifact, result, ..
                    } => {
                        match &executed_artifact.steps[1] {
                            SavedTrailStep::Comparison {
                                artifact: expected_artifact,
                            } => {
                                assert_eq!(artifact.as_ref(), expected_artifact.as_ref());
                            }
                            _ => panic!("expected second trail step artifact to be comparison"),
                        }
                        assert_eq!(result.as_ref(), &expected_comparison);
                    }
                    other => panic!(
                        "expected second replay step to be comparison, got {:?}",
                        other
                    ),
                }
                match replay.detached_step.as_deref() {
                    Some(TrailReplayStepResult::LensView {
                        artifact, result, ..
                    }) => {
                        match executed_artifact.detached_step.as_deref() {
                            Some(SavedTrailStep::LensView {
                                artifact: expected_artifact,
                            }) => {
                                assert_eq!(artifact.as_ref(), expected_artifact.as_ref());
                            }
                            _ => panic!("expected detached trail step artifact to be lens-view"),
                        }
                        assert_eq!(result.as_ref(), &expected_right_structure);
                    }
                    other => panic!("expected detached replay step, got {:?}", other),
                }
            }
            payload => panic!("expected trail execution, got {:?}", payload.kind()),
        }
    }

    #[test]
    fn artifact_rpc_reports_missing_and_invalid_artifacts() {
        let (_workspace, mut state, _target_key) = indexed_state();

        let padded_error = exploration_artifact(&mut state, json!({ "artifact_id": " missing " }))
            .expect_err("padded artifact id should be rejected");
        assert_eq!(
            padded_error.into_inner().message,
            "artifact_id must not have leading or trailing whitespace"
        );

        for operation in [
            exploration_artifact(&mut state, json!({ "artifact_id": "missing" })),
            execute_exploration_artifact(&mut state, json!({ "artifact_id": "missing" })),
            delete_exploration_artifact(&mut state, json!({ "artifact_id": "missing" })),
        ] {
            let error = operation.expect_err("missing artifact should be rejected");
            assert_eq!(
                error.into_inner().message,
                "unknown exploration artifact: missing"
            );
        }
    }

    #[test]
    fn save_exploration_artifact_rpc_rejects_invalid_artifacts() {
        let (_workspace, mut state, target_key) = indexed_state();
        let invalid = SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: "invalid-trail".to_owned(),
                title: "Invalid Trail".to_owned(),
                summary: None,
            },
            payload: ExplorationArtifactPayload::Trail {
                artifact: Box::new(SavedTrailArtifact {
                    steps: vec![SavedTrailStep::LensView {
                        artifact: Box::new(SavedLensViewArtifact {
                            root_node_key: target_key.clone(),
                            current_node_key: target_key,
                            lens: ExplorationLens::Structure,
                            limit: 20,
                            unique: false,
                            frozen_context: false,
                        }),
                    }],
                    cursor: 1,
                    detached_step: None,
                }),
            },
        };

        let error = save_exploration_artifact(&mut state, json!({ "artifact": invalid }))
            .expect_err("save artifact RPC should reject malformed artifacts");
        assert_eq!(
            error.into_inner().message,
            "trail cursor must point to an existing step"
        );
    }

    #[test]
    fn save_exploration_artifact_rpc_respects_non_overwrite_policy() {
        let (_workspace, mut state, focus_key) = non_obvious_state();
        let original = saved_lens_artifact(
            "saved-unresolved",
            "Original",
            &focus_key,
            ExplorationLens::Refs,
        );
        let replacement = saved_lens_artifact(
            "saved-unresolved",
            "Replacement",
            &focus_key,
            ExplorationLens::Unresolved,
        );

        let _: SaveExplorationArtifactResult = serde_json::from_value(
            save_exploration_artifact(
                &mut state,
                json!({ "artifact": original.clone(), "overwrite": true }),
            )
            .expect("initial save should succeed"),
        )
        .expect("save result should decode");

        let error = save_exploration_artifact(
            &mut state,
            json!({ "artifact": replacement, "overwrite": false }),
        )
        .expect_err("non-overwrite save should reject replacement");
        assert_eq!(
            error.into_inner().message,
            "exploration artifact already exists: saved-unresolved"
        );

        let stored = state
            .database
            .exploration_artifact("saved-unresolved")
            .expect("stored artifact lookup should succeed")
            .expect("stored artifact should remain readable");
        assert_eq!(stored, original);
    }

    #[test]
    fn workbench_pack_rpc_round_trips_import_validate_export_delete_after_reopen() {
        let (_workspace, mut state, _target_key) = indexed_state();
        let pack = sample_workbench_pack("pack/research-review", "Research Review Pack");

        let validation: ValidateWorkbenchPackResult = serde_json::from_value(
            validate_workbench_pack(&mut state, json!({ "pack": pack.clone() }))
                .expect("validate pack RPC should succeed"),
        )
        .expect("validation result should decode");
        assert!(validation.valid);
        assert!(validation.issues.is_empty());
        assert_eq!(validation.pack, Some(pack.summary()));
        let listed_after_validation: ListWorkbenchPacksResult = serde_json::from_value(
            list_workbench_packs(&mut state, json!({}))
                .expect("validation should not persist packs"),
        )
        .expect("list result should decode");
        assert!(listed_after_validation.packs.is_empty());

        let imported: ImportWorkbenchPackResult = serde_json::from_value(
            import_workbench_pack(&mut state, json!({ "pack": pack.clone() }))
                .expect("import pack RPC should succeed"),
        )
        .expect("import result should decode");
        assert_eq!(imported.pack, pack.summary());

        let conflict = import_workbench_pack(&mut state, json!({ "pack": pack.clone() }))
            .expect_err("default import should reject existing packs");
        assert_eq!(
            conflict.into_inner().message,
            "workbench pack already exists: pack/research-review"
        );

        let mut replacement = pack.clone();
        replacement.metadata.title = "Research Review Pack Updated".to_owned();
        let overwritten: ImportWorkbenchPackResult = serde_json::from_value(
            import_workbench_pack(
                &mut state,
                json!({ "pack": replacement.clone(), "overwrite": true }),
            )
            .expect("overwrite import should succeed"),
        )
        .expect("overwrite result should decode");
        assert_eq!(overwritten.pack, replacement.summary());

        let root = state.root.clone();
        let db_path = state.db_path.clone();
        let discovery = state.discovery.clone();
        drop(state);

        let mut reopened =
            ServerState::new(root, db_path, Vec::new(), discovery).expect("state should reopen");
        let listed: ListWorkbenchPacksResult = serde_json::from_value(
            list_workbench_packs(&mut reopened, json!({}))
                .expect("list packs after reopen should succeed"),
        )
        .expect("list result should decode");
        assert_eq!(listed.packs, vec![replacement.summary()]);

        let shown: WorkbenchPackResult = serde_json::from_value(
            workbench_pack(&mut reopened, json!({ "pack_id": "pack/research-review" }))
                .expect("show pack after reopen should succeed"),
        )
        .expect("show result should decode");
        assert_eq!(shown.pack, replacement);

        let exported: WorkbenchPackManifest = serde_json::from_value(
            export_workbench_pack(&mut reopened, json!({ "pack_id": "pack/research-review" }))
                .expect("export pack after reopen should succeed"),
        )
        .expect("exported pack should decode as canonical manifest");
        assert_eq!(exported, shown.pack);

        let deleted: DeleteWorkbenchPackResult = serde_json::from_value(
            delete_workbench_pack(&mut reopened, json!({ "pack_id": "pack/research-review" }))
                .expect("delete pack RPC should succeed"),
        )
        .expect("delete result should decode");
        assert_eq!(deleted.pack_id, "pack/research-review");
        let listed_after_delete: ListWorkbenchPacksResult = serde_json::from_value(
            list_workbench_packs(&mut reopened, json!({}))
                .expect("list after delete should succeed"),
        )
        .expect("list result should decode");
        assert!(listed_after_delete.packs.is_empty());
    }

    #[test]
    fn imported_pack_workflows_are_visible_through_live_catalog_handlers() {
        let (_workspace, mut state, target_key) = indexed_state();
        let mut pack = sample_workbench_pack("pack/workflows", "Pack Workflows");
        pack.workflows.push(sample_pack_workflow(
            "workflow/pack/live",
            "Pack Live Workflow",
            &target_key,
        ));
        let _: ImportWorkbenchPackResult = serde_json::from_value(
            import_workbench_pack(&mut state, json!({ "pack": pack }))
                .expect("pack import should succeed"),
        )
        .expect("import result should decode");

        let listed: ListWorkflowsResult = serde_json::from_value(
            list_workflows(&mut state, json!({})).expect("list workflows should succeed"),
        )
        .expect("workflow list should decode");
        assert!(listed.issues.is_empty());
        assert!(listed.workflows.iter().any(|workflow| {
            workflow.metadata.workflow_id == "workflow/pack/live"
                && workflow.metadata.title == "Pack Live Workflow"
        }));

        let shown: WorkflowResult = serde_json::from_value(
            workflow(&mut state, json!({ "workflow_id": "workflow/pack/live" }))
                .expect("pack workflow should be inspectable"),
        )
        .expect("workflow result should decode");
        assert_eq!(shown.workflow.metadata.title, "Pack Live Workflow");

        let run: RunWorkflowResult = serde_json::from_value(
            run_workflow(&mut state, json!({ "workflow_id": "workflow/pack/live" }))
                .expect("pack workflow should execute"),
        )
        .expect("workflow run should decode");
        assert_eq!(
            run.result.workflow.metadata.workflow_id,
            "workflow/pack/live"
        );
        assert_eq!(run.result.steps.len(), 1);
    }

    #[test]
    fn review_routine_rpc_executes_audit_routines_with_save_compare_and_profiles() {
        let (_workspace, mut state) = audit_state();
        let _: SaveCorpusAuditReviewResult = serde_json::from_value(
            save_corpus_audit_review(
                &mut state,
                json!({
                    "audit": "duplicate-titles",
                    "limit": 20,
                    "review_id": "review/routine/z-old"
                }),
            )
            .expect("previous audit review should save"),
        )
        .expect("previous save result should decode");
        sleep(Duration::from_millis(20));
        let _: SaveCorpusAuditReviewResult = serde_json::from_value(
            save_corpus_audit_review(
                &mut state,
                json!({
                    "audit": "duplicate-titles",
                    "limit": 20,
                    "review_id": "review/routine/a-new"
                }),
            )
            .expect("newer audit review should save"),
        )
        .expect("newer save result should decode");

        let mut pack = sample_workbench_pack("pack/routines", "Routine Pack");
        pack.report_profiles
            .push(sample_routine_report_profile("profile/routine/detail"));
        pack.report_profiles
            .push(sample_routine_only_review_profile(
                "profile/routine/review-lines",
            ));
        let mut routine =
            sample_audit_review_routine("routine/pack/audit", "profile/routine/detail");
        routine
            .report_profile_ids
            .push("profile/routine/review-lines".to_owned());
        pack.review_routines.push(routine);
        let _: ImportWorkbenchPackResult = serde_json::from_value(
            import_workbench_pack(&mut state, json!({ "pack": pack }))
                .expect("routine pack should import"),
        )
        .expect("pack import result should decode");

        let run: RunReviewRoutineResult = serde_json::from_value(
            run_review_routine(&mut state, json!({ "routine_id": "routine/pack/audit" }))
                .expect("routine should run"),
        )
        .expect("routine result should decode");

        assert_eq!(run.result.routine.metadata.routine_id, "routine/pack/audit");
        match &run.result.source {
            ReviewRoutineSourceExecutionResult::Audit { result } => {
                assert_eq!(result.audit, CorpusAuditKind::DuplicateTitles);
                assert_eq!(result.entries.len(), 1);
            }
            other => panic!("expected audit routine source, got {other:?}"),
        }
        assert_eq!(
            run.result
                .saved_review
                .as_ref()
                .expect("routine should save a review")
                .metadata
                .review_id,
            "review/routine/001-current"
        );
        let compare = run
            .result
            .compare
            .as_ref()
            .expect("routine should return compare result");
        assert_eq!(
            compare
                .base_review
                .as_ref()
                .expect("previous compatible review should be selected")
                .metadata
                .review_id,
            "review/routine/a-new"
        );
        assert_eq!(
            compare
                .diff
                .as_ref()
                .expect("compatible reviews should diff")
                .unchanged
                .len(),
            1
        );
        assert!(
            compare
                .report
                .as_ref()
                .expect("compare profile should be applied")
                .lines
                .iter()
                .any(|line| matches!(line, ReviewRoutineReportLine::Unchanged { .. }))
        );

        let profile = run
            .result
            .reports
            .first()
            .expect("routine report profile should be applied");
        assert_eq!(
            profile.profile.metadata.profile_id,
            "profile/routine/detail"
        );
        assert!(
            profile
                .lines
                .iter()
                .any(|line| matches!(line, ReviewRoutineReportLine::Routine { .. }))
        );
        assert!(
            profile
                .lines
                .iter()
                .any(|line| matches!(line, ReviewRoutineReportLine::Entry { .. }))
        );
        assert!(
            profile
                .lines
                .iter()
                .any(|line| matches!(line, ReviewRoutineReportLine::Finding { .. }))
        );
        assert!(
            profile
                .lines
                .iter()
                .any(|line| matches!(line, ReviewRoutineReportLine::Diff { .. }))
        );
        let routine_only_profile = run
            .result
            .reports
            .iter()
            .find(|report| report.profile.metadata.profile_id == "profile/routine/review-lines")
            .expect("routine-only review profile should be applied");
        assert!(
            routine_only_profile
                .lines
                .iter()
                .any(|line| matches!(line, ReviewRoutineReportLine::Review { .. }))
        );
        assert!(
            routine_only_profile
                .lines
                .iter()
                .any(|line| matches!(line, ReviewRoutineReportLine::Finding { .. }))
        );
        assert!(routine_only_profile.lines.iter().all(|line| matches!(
            line,
            ReviewRoutineReportLine::Review { .. } | ReviewRoutineReportLine::Finding { .. }
        )));

        let saved: ReviewRunResult = serde_json::from_value(
            review_run(
                &mut state,
                json!({ "review_id": "review/routine/001-current" }),
            )
            .expect("saved routine review should be loadable"),
        )
        .expect("saved review should decode");
        assert_eq!(saved.review.findings.len(), 1);
    }

    #[test]
    fn review_routine_rpc_executes_workflow_routines_with_inputs_and_reports_step_failures() {
        let (_workspace, mut state, target_key) = indexed_state();
        let mut pack = sample_workbench_pack("pack/workflow-routine", "Workflow Routine Pack");
        pack.workflows.push(sample_input_workflow(
            "workflow/pack/input-review",
            "Input Review",
        ));
        pack.review_routines.push(sample_workflow_review_routine(
            "routine/pack/workflow",
            "workflow/pack/input-review",
            Some("review/routine/workflow"),
        ));
        pack.review_routines.push(sample_workflow_review_routine(
            "routine/pack/workflow-failure",
            "workflow/pack/input-review",
            None,
        ));
        let _: ImportWorkbenchPackResult = serde_json::from_value(
            import_workbench_pack(&mut state, json!({ "pack": pack }))
                .expect("workflow routine pack should import"),
        )
        .expect("pack import result should decode");

        let run: RunReviewRoutineResult = serde_json::from_value(
            run_review_routine(
                &mut state,
                json!({
                    "routine_id": "routine/pack/workflow",
                    "inputs": [{
                        "input_id": "focus",
                        "kind": "node-key",
                        "node_key": target_key
                    }]
                }),
            )
            .expect("workflow routine should run"),
        )
        .expect("workflow routine result should decode");
        match &run.result.source {
            ReviewRoutineSourceExecutionResult::Workflow { result } => {
                assert_eq!(
                    result.workflow.metadata.workflow_id,
                    "workflow/pack/input-review"
                );
                assert_eq!(result.steps.len(), 2);
            }
            other => panic!("expected workflow routine source, got {other:?}"),
        }
        assert_eq!(
            run.result
                .saved_review
                .as_ref()
                .expect("workflow routine should save review")
                .metadata
                .review_id,
            "review/routine/workflow"
        );

        let missing_input = run_review_routine(
            &mut state,
            json!({ "routine_id": "routine/pack/workflow-failure" }),
        )
        .expect_err("missing workflow input should fail before execution");
        assert_eq!(
            missing_input.into_inner().message,
            "workflow input focus must be assigned"
        );

        let step_failure = run_review_routine(
            &mut state,
            json!({
                "routine_id": "routine/pack/workflow-failure",
                "inputs": [{
                    "input_id": "focus",
                    "kind": "node-key",
                    "node_key": "missing:node"
                }]
            }),
        )
        .expect_err("workflow step failure should be surfaced with context");
        assert_eq!(
            step_failure.into_inner().message,
            "workflow step resolve-focus failed: unknown workflow note target: missing:node"
        );
    }

    #[test]
    fn review_routine_save_review_conflicts_prevent_workflow_side_effects() {
        let (_workspace, mut state, target_key) = indexed_state();
        let _: SaveReviewRunResult = serde_json::from_value(
            save_review_run(
                &mut state,
                json!({
                    "review": sample_audit_review_run(
                        "review/routine/conflict",
                        "Existing Routine Review",
                        ReviewFindingStatus::Open
                    )
                }),
            )
            .expect("existing review should save"),
        )
        .expect("save review result should decode");

        let mut pack = sample_workbench_pack("pack/routine-conflict", "Routine Conflict Pack");
        pack.workflows.push(sample_artifact_save_workflow(
            "workflow/pack/conflict-side-effect",
            &target_key,
        ));
        pack.review_routines.push(ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: "routine/pack/conflict".to_owned(),
                title: "Conflict Routine".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Workflow {
                workflow_id: "workflow/pack/conflict-side-effect".to_owned(),
            },
            inputs: Vec::new(),
            save_review: ReviewRoutineSaveReviewPolicy {
                enabled: true,
                review_id: Some("review/routine/conflict".to_owned()),
                title: None,
                summary: None,
                overwrite: false,
            },
            compare: None,
            report_profile_ids: Vec::new(),
        });
        let _: ImportWorkbenchPackResult = serde_json::from_value(
            import_workbench_pack(&mut state, json!({ "pack": pack }))
                .expect("conflict pack should import"),
        )
        .expect("pack import result should decode");

        let conflict =
            run_review_routine(&mut state, json!({ "routine_id": "routine/pack/conflict" }))
                .expect_err("review conflict should reject before workflow execution");
        assert_eq!(
            conflict.into_inner().message,
            "review run already exists: review/routine/conflict"
        );
        assert!(
            state
                .database
                .exploration_artifact("routine-conflict-artifact")
                .expect("artifact lookup should succeed")
                .is_none(),
            "workflow artifact-save side effect should not run on review conflict"
        );
    }

    #[test]
    fn review_routine_rpc_reports_unknown_routines() {
        let (_workspace, mut state) = audit_state();
        let error = run_review_routine(&mut state, json!({ "routine_id": "routine/missing" }))
            .expect_err("unknown routine should fail");
        assert_eq!(
            error.into_inner().message,
            "unknown review routine: routine/missing"
        );
    }

    #[test]
    fn workbench_pack_rpc_reports_malformed_unsupported_and_missing_packs() {
        let (_workspace, mut state, _target_key) = indexed_state();
        let valid = sample_workbench_pack("pack/research-review", "Research Review Pack");

        let padded_error =
            workbench_pack(&mut state, json!({ "pack_id": " pack/research-review " }))
                .expect_err("padded pack id should be rejected");
        assert_eq!(
            padded_error.into_inner().message,
            "pack_id must not have leading or trailing whitespace"
        );

        for operation in [
            workbench_pack(&mut state, json!({ "pack_id": "pack/missing" })),
            export_workbench_pack(&mut state, json!({ "pack_id": "pack/missing" })),
            delete_workbench_pack(&mut state, json!({ "pack_id": "pack/missing" })),
        ] {
            let error = operation.expect_err("missing pack should be rejected");
            assert_eq!(
                error.into_inner().message,
                "unknown workbench pack: pack/missing"
            );
        }

        let mut unsupported = valid.clone();
        unsupported.compatibility = WorkbenchPackCompatibility { version: 2 };
        let validation: ValidateWorkbenchPackResult = serde_json::from_value(
            validate_workbench_pack(&mut state, json!({ "pack": unsupported }))
                .expect("unsupported version should produce validation issues"),
        )
        .expect("validation result should decode");
        assert!(!validation.valid);
        assert_eq!(
            validation.issues[0].kind,
            WorkbenchPackIssueKind::UnsupportedVersion
        );
        assert_eq!(
            validation.issues[0].message,
            "unsupported workbench pack compatibility version 2; supported version is 1"
        );

        let future_syntax_validation: ValidateWorkbenchPackResult = serde_json::from_value(
            validate_workbench_pack(
                &mut state,
                json!({
                    "pack": {
                        "pack_id": "pack/future",
                        "title": "Future Pack",
                        "compatibility": { "version": 2 },
                        "workflows": [{
                            "workflow_id": "workflow/future",
                            "title": "Future Workflow",
                            "inputs": [{
                                "input_id": "focus",
                                "title": "Focus",
                                "kind": "future-target"
                            }],
                            "steps": []
                        }]
                    }
                }),
            )
            .expect("future pack compatibility should be detected before typed parse"),
        )
        .expect("future syntax validation result should decode");
        assert!(!future_syntax_validation.valid);
        assert_eq!(future_syntax_validation.pack, None);
        assert_eq!(
            future_syntax_validation.issues[0].kind,
            WorkbenchPackIssueKind::UnsupportedVersion
        );

        let future_import_error = import_workbench_pack(
            &mut state,
            json!({
                "pack": {
                    "pack_id": "pack/future",
                    "title": "Future Pack",
                    "compatibility": { "version": 2 },
                    "workflows": [{
                        "workflow_id": "workflow/future",
                        "title": "Future Workflow",
                        "inputs": [{
                            "input_id": "focus",
                            "title": "Focus",
                            "kind": "future-target"
                        }],
                        "steps": []
                    }]
                }
            }),
        )
        .expect_err("future pack import should fail as unsupported before typed parse");
        assert_eq!(
            future_import_error.into_inner().message,
            "unsupported workbench pack compatibility version 2; supported version is 1"
        );

        let listed_after_validation: ListWorkbenchPacksResult = serde_json::from_value(
            list_workbench_packs(&mut state, json!({}))
                .expect("invalid validation should not persist packs"),
        )
        .expect("list result should decode");
        assert!(listed_after_validation.packs.is_empty());

        let mut empty = valid.clone();
        empty.workflows.clear();
        empty.review_routines.clear();
        empty.report_profiles.clear();
        let save_error = import_workbench_pack(&mut state, json!({ "pack": empty }))
            .expect_err("import should reject invalid packs");
        assert_eq!(
            save_error.into_inner().message,
            "workbench packs must contain at least one workflow, review routine, or report profile"
        );

        let malformed_error = validate_workbench_pack(
            &mut state,
            json!({
                "pack": {
                    "pack_id": "pack/malformed",
                    "title": "Malformed",
                    "report_profiles": [{
                        "profile_id": "profile/malformed",
                        "title": "Malformed",
                        "subjects": ["future-subject"]
                    }]
                }
            }),
        )
        .expect_err("malformed pack should fail request parsing");
        assert!(
            malformed_error
                .into_inner()
                .message
                .starts_with("invalid request parameters:"),
            "unexpected malformed error"
        );
    }

    #[test]
    fn review_run_rpc_operations_round_trip_and_mark_review_runs() {
        let (_workspace, mut state) = audit_state();
        let review = sample_audit_review_run(
            "review/audit/dangling-links",
            "Dangling Link Review",
            ReviewFindingStatus::Open,
        );

        let saved: SaveReviewRunResult = serde_json::from_value(
            save_review_run(
                &mut state,
                json!({ "review": review.clone(), "overwrite": true }),
            )
            .expect("save review RPC should succeed"),
        )
        .expect("save review result should decode");
        assert_eq!(saved.review.metadata, review.metadata);
        assert_eq!(saved.review.finding_count, 1);
        assert_eq!(saved.review.status_counts.open, 1);

        let listed: ListReviewRunsResult = serde_json::from_value(
            list_review_runs(&mut state, json!({})).expect("list reviews RPC should succeed"),
        )
        .expect("list reviews result should decode");
        assert_eq!(listed.reviews, vec![saved.review.clone()]);

        let inspected: ReviewRunResult = serde_json::from_value(
            review_run(
                &mut state,
                json!({ "review_id": "review/audit/dangling-links" }),
            )
            .expect("inspect review RPC should succeed"),
        )
        .expect("inspect review result should decode");
        assert_eq!(inspected.review, review);

        let marked: MarkReviewFindingResult = serde_json::from_value(
            mark_review_finding(
                &mut state,
                json!({
                    "review_id": "review/audit/dangling-links",
                    "finding_id": "audit/dangling-links/source/missing-id",
                    "status": "reviewed"
                }),
            )
            .expect("mark review finding RPC should succeed"),
        )
        .expect("mark result should decode");
        assert_eq!(marked.transition.from_status, ReviewFindingStatus::Open);
        assert_eq!(marked.transition.to_status, ReviewFindingStatus::Reviewed);

        let root = state.root.clone();
        let db_path = state.db_path.clone();
        let discovery = state.discovery.clone();
        drop(state);

        let mut reopened =
            ServerState::new(root, db_path, Vec::new(), discovery).expect("state should reopen");
        let updated: ReviewRunResult = serde_json::from_value(
            review_run(
                &mut reopened,
                json!({ "review_id": "review/audit/dangling-links" }),
            )
            .expect("marked review should load after reopen"),
        )
        .expect("marked review result should decode");
        assert_eq!(
            updated.review.findings[0].status,
            ReviewFindingStatus::Reviewed
        );

        let deleted: DeleteReviewRunResult = serde_json::from_value(
            delete_review_run(
                &mut reopened,
                json!({ "review_id": "review/audit/dangling-links" }),
            )
            .expect("delete review RPC should succeed"),
        )
        .expect("delete review result should decode");
        assert_eq!(deleted.review_id, "review/audit/dangling-links");

        let listed_after_delete: ListReviewRunsResult = serde_json::from_value(
            list_review_runs(&mut reopened, json!({}))
                .expect("list reviews after delete should succeed"),
        )
        .expect("list reviews after delete should decode");
        assert!(listed_after_delete.reviews.is_empty());
    }

    #[test]
    fn review_run_diff_rpc_classifies_stored_review_runs() {
        let (_workspace, mut state) = audit_state();
        let mut base = sample_audit_review_run(
            "review/audit/dangling-links/base",
            "Base Dangling Review",
            ReviewFindingStatus::Open,
        );
        base.findings.push(ReviewFinding {
            finding_id: "audit/dangling-links/source/removed-id".to_owned(),
            status: ReviewFindingStatus::Dismissed,
            payload: ReviewFindingPayload::Audit {
                entry: Box::new(CorpusAuditEntry::DanglingLink {
                    record: Box::new(DanglingLinkAuditRecord {
                        source: AnchorRecord {
                            node_key: "file:source.org".to_owned(),
                            explicit_id: Some("source-id".to_owned()),
                            file_path: "source.org".to_owned(),
                            title: "Source".to_owned(),
                            outline_path: "Source".to_owned(),
                            aliases: Vec::new(),
                            tags: Vec::new(),
                            refs: Vec::new(),
                            todo_keyword: None,
                            scheduled_for: None,
                            deadline_for: None,
                            closed_at: None,
                            level: 0,
                            line: 1,
                            kind: NodeKind::File,
                            file_mtime_ns: 0,
                            backlink_count: 0,
                            forward_link_count: 0,
                        },
                        missing_explicit_id: "removed-id".to_owned(),
                        line: 14,
                        column: 3,
                        preview: "[[id:removed-id][Removed]]".to_owned(),
                    }),
                }),
            },
        });
        let mut target = sample_audit_review_run(
            "review/audit/dangling-links/target",
            "Target Dangling Review",
            ReviewFindingStatus::Reviewed,
        );
        target.findings.push(ReviewFinding {
            finding_id: "audit/dangling-links/source/added-id".to_owned(),
            status: ReviewFindingStatus::Open,
            payload: ReviewFindingPayload::Audit {
                entry: Box::new(CorpusAuditEntry::DanglingLink {
                    record: Box::new(DanglingLinkAuditRecord {
                        source: AnchorRecord {
                            node_key: "file:source.org".to_owned(),
                            explicit_id: Some("source-id".to_owned()),
                            file_path: "source.org".to_owned(),
                            title: "Source".to_owned(),
                            outline_path: "Source".to_owned(),
                            aliases: Vec::new(),
                            tags: Vec::new(),
                            refs: Vec::new(),
                            todo_keyword: None,
                            scheduled_for: None,
                            deadline_for: None,
                            closed_at: None,
                            level: 0,
                            line: 1,
                            kind: NodeKind::File,
                            file_mtime_ns: 0,
                            backlink_count: 0,
                            forward_link_count: 0,
                        },
                        missing_explicit_id: "added-id".to_owned(),
                        line: 16,
                        column: 3,
                        preview: "[[id:added-id][Added]]".to_owned(),
                    }),
                }),
            },
        });

        state
            .database
            .save_review_run(&base)
            .expect("base review should be saved");
        state
            .database
            .save_review_run(&target)
            .expect("target review should be saved");

        let diff: ReviewRunDiffResult = serde_json::from_value(
            diff_review_runs(
                &mut state,
                json!({
                    "base_review_id": "review/audit/dangling-links/base",
                    "target_review_id": "review/audit/dangling-links/target"
                }),
            )
            .expect("review diff RPC should succeed"),
        )
        .expect("diff result should decode");

        assert_eq!(diff.diff.added.len(), 1);
        assert_eq!(
            diff.diff.added[0].finding_id,
            "audit/dangling-links/source/added-id"
        );
        assert_eq!(diff.diff.removed.len(), 1);
        assert_eq!(
            diff.diff.removed[0].finding_id,
            "audit/dangling-links/source/removed-id"
        );
        assert!(diff.diff.unchanged.is_empty());
        assert_eq!(diff.diff.status_changed.len(), 1);
        assert_eq!(
            diff.diff.status_changed[0].finding_id,
            "audit/dangling-links/source/missing-id"
        );
        assert_eq!(
            diff.diff.status_changed[0].from_status,
            ReviewFindingStatus::Open
        );
        assert_eq!(
            diff.diff.status_changed[0].to_status,
            ReviewFindingStatus::Reviewed
        );

        let incompatible = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/audit/orphans".to_owned(),
                title: "Orphan Review".to_owned(),
                summary: None,
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::OrphanNotes,
                limit: 200,
            },
            findings: Vec::new(),
        };
        state
            .database
            .save_review_run(&incompatible)
            .expect("incompatible review should be saved");
        let error = diff_review_runs(
            &mut state,
            json!({
                "base_review_id": "review/audit/dangling-links/base",
                "target_review_id": "review/audit/orphans"
            }),
        )
        .expect_err("incompatible review diff should be rejected");
        assert!(error.into_inner().message.contains("different audit kinds"));
    }

    #[test]
    fn review_finding_remediation_preview_reports_supported_audit_evidence_without_mutation() {
        let (_workspace, mut state) = audit_state();
        let source_path = state.root.join("dangling-source.org");
        let source_before = fs::read_to_string(&source_path).expect("source file should read");

        let _: SaveCorpusAuditReviewResult = serde_json::from_value(
            save_corpus_audit_review(
                &mut state,
                json!({
                    "audit": "dangling-links",
                    "limit": 20,
                    "review_id": "review/audit/dangling-links/custom",
                    "overwrite": true
                }),
            )
            .expect("save audit review RPC should succeed"),
        )
        .expect("save audit review result should decode");
        let stored_before: ReviewRunResult = serde_json::from_value(
            review_run(
                &mut state,
                json!({ "review_id": "review/audit/dangling-links/custom" }),
            )
            .expect("saved audit review should load"),
        )
        .expect("review result should decode");
        let dangling_finding_id = stored_before.review.findings[0].finding_id.clone();

        let preview: ReviewFindingRemediationPreviewResult = serde_json::from_value(
            review_finding_remediation_preview(
                &mut state,
                json!({
                    "review_id": "review/audit/dangling-links/custom",
                    "finding_id": dangling_finding_id
                }),
            )
            .expect("dangling-link preview should succeed"),
        )
        .expect("preview result should decode");
        assert_eq!(
            preview.preview.review_id,
            "review/audit/dangling-links/custom"
        );
        assert_eq!(preview.preview.status, ReviewFindingStatus::Open);
        match preview.preview.payload {
            AuditRemediationPreviewPayload::DanglingLink {
                source,
                missing_explicit_id,
                file_path,
                line,
                column,
                preview,
                suggestion,
                confidence,
                reason,
            } => {
                assert_eq!(source.explicit_id.as_deref(), Some("dangling-source-id"));
                assert_eq!(missing_explicit_id, "missing-id");
                assert_eq!(file_path, "dangling-source.org");
                assert_eq!(line, 6);
                assert!(column > 0);
                assert!(preview.contains("missing-id"));
                assert!(suggestion.contains("id:missing-id"));
                assert_eq!(confidence, AuditRemediationConfidence::Medium);
                assert!(reason.contains("dangling-source.org"));
            }
            other => panic!("expected dangling-link preview, got {other:?}"),
        }

        let stored_after: ReviewRunResult = serde_json::from_value(
            review_run(
                &mut state,
                json!({ "review_id": "review/audit/dangling-links/custom" }),
            )
            .expect("saved audit review should still load"),
        )
        .expect("review result should decode");
        assert_eq!(stored_after.review, stored_before.review);
        assert_eq!(
            fs::read_to_string(&source_path).expect("source file should still read"),
            source_before
        );

        let _: SaveCorpusAuditReviewResult = serde_json::from_value(
            save_corpus_audit_review(
                &mut state,
                json!({
                    "audit": "duplicate-titles",
                    "limit": 20,
                    "review_id": "review/audit/duplicate-titles/custom",
                    "overwrite": true
                }),
            )
            .expect("save duplicate-title review RPC should succeed"),
        )
        .expect("duplicate-title review result should decode");
        let duplicate_review: ReviewRunResult = serde_json::from_value(
            review_run(
                &mut state,
                json!({ "review_id": "review/audit/duplicate-titles/custom" }),
            )
            .expect("duplicate-title review should load"),
        )
        .expect("duplicate-title review result should decode");
        let duplicate_preview: ReviewFindingRemediationPreviewResult = serde_json::from_value(
            review_finding_remediation_preview(
                &mut state,
                json!({
                    "review_id": "review/audit/duplicate-titles/custom",
                    "finding_id": duplicate_review.review.findings[0].finding_id
                }),
            )
            .expect("duplicate-title preview should succeed"),
        )
        .expect("duplicate-title preview result should decode");
        match duplicate_preview.preview.payload {
            AuditRemediationPreviewPayload::DuplicateTitle {
                title,
                notes,
                suggestion,
                confidence,
                reason,
            } => {
                assert_eq!(title, "Shared Title");
                assert_eq!(notes.len(), 2);
                assert!(suggestion.contains("Disambiguate"));
                assert_eq!(confidence, AuditRemediationConfidence::High);
                assert!(reason.contains("2 notes"));
            }
            other => panic!("expected duplicate-title preview, got {other:?}"),
        }

        let _: SaveCorpusAuditReviewResult = serde_json::from_value(
            save_corpus_audit_review(
                &mut state,
                json!({
                    "audit": "orphan-notes",
                    "limit": 20,
                    "review_id": "review/audit/orphan-notes/custom",
                    "overwrite": true
                }),
            )
            .expect("save orphan review RPC should succeed"),
        )
        .expect("orphan review result should decode");
        let orphan_review: ReviewRunResult = serde_json::from_value(
            review_run(
                &mut state,
                json!({ "review_id": "review/audit/orphan-notes/custom" }),
            )
            .expect("orphan review should load"),
        )
        .expect("orphan review result should decode");
        let unsupported = review_finding_remediation_preview(
            &mut state,
            json!({
                "review_id": "review/audit/orphan-notes/custom",
                "finding_id": orphan_review.review.findings[0].finding_id
            }),
        )
        .expect_err("orphan preview should be rejected");
        assert_eq!(
            unsupported.into_inner().message,
            "review finding has no remediation preview for orphan-note evidence"
        );
    }

    #[test]
    fn review_run_rpc_reports_missing_invalid_and_malformed_reviews() {
        let (_workspace, mut state) = audit_state();
        let review = sample_audit_review_run(
            "review/audit/dangling-links",
            "Dangling Link Review",
            ReviewFindingStatus::Open,
        );

        let padded_error = review_run(&mut state, json!({ "review_id": " missing " }))
            .expect_err("padded review id should be rejected");
        assert_eq!(
            padded_error.into_inner().message,
            "review_id must not have leading or trailing whitespace"
        );
        let padded_diff = diff_review_runs(
            &mut state,
            json!({
                "base_review_id": " missing ",
                "target_review_id": "review/audit/dangling-links"
            }),
        )
        .expect_err("padded review id in diff should be rejected");
        assert_eq!(
            padded_diff.into_inner().message,
            "review_id must not have leading or trailing whitespace"
        );

        for operation in [
            review_run(&mut state, json!({ "review_id": "missing" })),
            delete_review_run(&mut state, json!({ "review_id": "missing" })),
            mark_review_finding(
                &mut state,
                json!({
                    "review_id": "missing",
                    "finding_id": "finding",
                    "status": "reviewed"
                }),
            ),
            diff_review_runs(
                &mut state,
                json!({
                    "base_review_id": "missing",
                    "target_review_id": "review/audit/dangling-links"
                }),
            ),
        ] {
            let error = operation.expect_err("missing review should be rejected");
            assert_eq!(error.into_inner().message, "unknown review run: missing");
        }

        let invalid_review = ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: "review/audit/invalid".to_owned(),
                title: String::new(),
                summary: None,
            },
            ..review.clone()
        };
        let invalid_save = save_review_run(&mut state, json!({ "review": invalid_review }))
            .expect_err("invalid review should be rejected");
        assert_eq!(invalid_save.into_inner().message, "title must not be empty");

        let _: SaveReviewRunResult = serde_json::from_value(
            save_review_run(
                &mut state,
                json!({ "review": review.clone(), "overwrite": true }),
            )
            .expect("initial review save should succeed"),
        )
        .expect("save result should decode");

        let replacement = sample_audit_review_run(
            "review/audit/dangling-links",
            "Replacement",
            ReviewFindingStatus::Dismissed,
        );
        let overwrite_error = save_review_run(
            &mut state,
            json!({ "review": replacement, "overwrite": false }),
        )
        .expect_err("non-overwrite review save should reject replacement");
        assert_eq!(
            overwrite_error.into_inner().message,
            "review run already exists: review/audit/dangling-links"
        );

        let unknown_finding = mark_review_finding(
            &mut state,
            json!({
                "review_id": "review/audit/dangling-links",
                "finding_id": "missing-finding",
                "status": "reviewed"
            }),
        )
        .expect_err("unknown finding should be rejected");
        assert_eq!(
            unknown_finding.into_inner().message,
            "unknown review finding missing-finding in review run review/audit/dangling-links"
        );

        let no_op = mark_review_finding(
            &mut state,
            json!({
                "review_id": "review/audit/dangling-links",
                "finding_id": "audit/dangling-links/source/missing-id",
                "status": "open"
            }),
        )
        .expect_err("no-op mark should be rejected");
        assert_eq!(
            no_op.into_inner().message,
            "review finding status transition must change status"
        );

        let db_file_name = state
            .db_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("db path should have UTF-8 file name");
        let malformed_path = state
            .db_path
            .with_file_name(format!("{db_file_name}.review-runs"))
            .join("v1")
            .join("malformed.json");
        fs::write(
            &malformed_path,
            serde_json::to_string_pretty(&json!({
                "review_id": "",
                "title": "Malformed",
                "kind": "audit",
                "audit": "dangling-links",
                "findings": []
            }))
            .expect("malformed review fixture should serialize"),
        )
        .expect("malformed review fixture should be written");

        let malformed = review_run(&mut state, json!({ "review_id": "malformed" }))
            .expect_err("malformed stored review should be rejected");
        assert!(
            malformed
                .into_inner()
                .message
                .contains("failed to load review run")
        );
    }

    #[test]
    fn save_corpus_audit_review_rpc_persists_typed_audit_evidence() {
        let (_workspace, mut state) = audit_state();

        let saved: SaveCorpusAuditReviewResult = serde_json::from_value(
            save_corpus_audit_review(
                &mut state,
                json!({
                    "audit": "dangling-links",
                    "limit": 20,
                    "review_id": "review/audit/dangling-links/custom",
                    "title": "Custom Dangling Review",
                    "overwrite": true
                }),
            )
            .expect("save audit review RPC should succeed"),
        )
        .expect("save audit review result should decode");
        assert_eq!(saved.result.audit, CorpusAuditKind::DanglingLinks);
        assert_eq!(saved.result.entries.len(), 1);
        assert_eq!(
            saved.review.metadata.review_id,
            "review/audit/dangling-links/custom"
        );
        assert_eq!(saved.review.finding_count, saved.result.entries.len());
        assert_eq!(saved.review.status_counts.open, saved.result.entries.len());

        let root = state.root.clone();
        let db_path = state.db_path.clone();
        let discovery = state.discovery.clone();
        drop(state);

        let mut reopened =
            ServerState::new(root, db_path, Vec::new(), discovery).expect("state should reopen");
        let inspected: ReviewRunResult = serde_json::from_value(
            review_run(
                &mut reopened,
                json!({ "review_id": "review/audit/dangling-links/custom" }),
            )
            .expect("saved audit review should load after reopen"),
        )
        .expect("review result should decode");
        assert_eq!(inspected.review.metadata.title, "Custom Dangling Review");
        match inspected.review.payload {
            ReviewRunPayload::Audit { audit, limit } => {
                assert_eq!(audit, CorpusAuditKind::DanglingLinks);
                assert_eq!(limit, 20);
            }
            other => panic!("expected audit review payload, got {:?}", other.kind()),
        }
        assert_eq!(inspected.review.findings.len(), saved.result.entries.len());
        match &saved.result.entries[0] {
            CorpusAuditEntry::DanglingLink { record } => {
                assert_eq!(
                    inspected.review.findings[0].finding_id,
                    format!(
                        "audit/dangling-links/{}/{}/{}/{}",
                        record.source.node_key,
                        record.missing_explicit_id,
                        record.line,
                        record.column
                    )
                );
            }
            other => panic!("expected dangling-link result, got {:?}", other.kind()),
        }
        assert_eq!(
            inspected.review.findings[0].status,
            ReviewFindingStatus::Open
        );
        match &inspected.review.findings[0].payload {
            ReviewFindingPayload::Audit { entry } => {
                assert_eq!(entry.as_ref(), &saved.result.entries[0]);
            }
            other => panic!("expected audit finding payload, got {:?}", other.kind()),
        }

        let conflict = save_corpus_audit_review(
            &mut reopened,
            json!({
                "audit": "dangling-links",
                "limit": 20,
                "review_id": "review/audit/dangling-links/custom",
                "overwrite": false
            }),
        )
        .expect_err("non-overwrite audit review save should reject replacement");
        assert_eq!(
            conflict.into_inner().message,
            "review run already exists: review/audit/dangling-links/custom"
        );
    }

    #[test]
    fn save_workflow_review_rpc_persists_typed_workflow_evidence() {
        let (_workspace, mut state, focus_key) = indexed_state();

        let saved: SaveWorkflowReviewResult = serde_json::from_value(
            save_workflow_review(
                &mut state,
                json!({
                    "workflow_id": BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
                    "inputs": [{
                        "input_id": "focus",
                        "kind": "node-key",
                        "node_key": focus_key
                    }],
                    "title": "Unresolved Sweep Review",
                    "overwrite": true
                }),
            )
            .expect("save workflow review RPC should succeed"),
        )
        .expect("save workflow review result should decode");
        assert_eq!(
            saved.result.workflow.metadata.workflow_id,
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
        );
        assert_eq!(saved.result.steps.len(), 4);
        assert_eq!(saved.review.finding_count, saved.result.steps.len());
        assert_eq!(saved.review.status_counts.open, saved.result.steps.len());
        assert!(
            saved
                .review
                .metadata
                .review_id
                .starts_with("review/workflow/builtin/unresolved-sweep/inputs-")
        );

        let root = state.root.clone();
        let db_path = state.db_path.clone();
        let discovery = state.discovery.clone();
        drop(state);

        let mut reopened =
            ServerState::new(root, db_path, Vec::new(), discovery).expect("state should reopen");
        let inspected: ReviewRunResult = serde_json::from_value(
            review_run(
                &mut reopened,
                json!({ "review_id": saved.review.metadata.review_id }),
            )
            .expect("saved workflow review should load after reopen"),
        )
        .expect("review result should decode");
        assert_eq!(inspected.review.metadata.title, "Unresolved Sweep Review");
        match &inspected.review.payload {
            ReviewRunPayload::Workflow {
                workflow,
                inputs,
                step_ids,
            } => {
                assert_eq!(
                    workflow.metadata.workflow_id,
                    BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
                );
                assert_eq!(inputs.len(), 1);
                assert_eq!(inputs[0].input_id, "focus");
                assert_eq!(
                    step_ids,
                    &saved
                        .result
                        .steps
                        .iter()
                        .map(|step| step.step_id.clone())
                        .collect::<Vec<_>>()
                );
            }
            other => panic!("expected workflow review payload, got {:?}", other.kind()),
        }
        assert_eq!(inspected.review.findings.len(), saved.result.steps.len());
        assert_eq!(
            inspected.review.findings[0].finding_id,
            format!("workflow-step/{}", saved.result.steps[0].step_id)
        );
        match &inspected.review.findings[0].payload {
            ReviewFindingPayload::WorkflowStep { step } => {
                assert_eq!(step.as_ref(), &saved.result.steps[0]);
            }
            other => panic!(
                "expected workflow-step finding payload, got {:?}",
                other.kind()
            ),
        }

        let unknown = save_workflow_review(
            &mut reopened,
            json!({
                "workflow_id": "workflow/builtin/missing",
                "overwrite": true
            }),
        )
        .expect_err("unknown workflow should be rejected");
        assert_eq!(
            unknown.into_inner().message,
            "unknown workflow: workflow/builtin/missing"
        );
    }

    #[test]
    fn save_workflow_review_rejects_existing_review_before_artifact_save_side_effects() {
        let (_workspace, mut state, left_key, right_key) = comparison_state();
        let workflow_dir = state.root.join("workflows");
        fs::create_dir_all(&workflow_dir).expect("workflow dir should be created");
        state.workflow_dirs = vec![workflow_dir.clone()];

        let workflow_id = "workflow/test/review-save-side-effect";
        let artifact_id = "workflow-review-side-effect";
        let workflow = WorkflowSpec {
            metadata: slipbox_core::WorkflowMetadata {
                workflow_id: workflow_id.to_owned(),
                title: "Review Save Side Effect".to_owned(),
                summary: Some("Would save an artifact if it reaches execution".to_owned()),
            },
            compatibility: slipbox_core::WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![
                slipbox_core::WorkflowStepSpec {
                    step_id: "resolve-left".to_owned(),
                    payload: slipbox_core::WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::NodeKey {
                            node_key: left_key.clone(),
                        },
                    },
                },
                slipbox_core::WorkflowStepSpec {
                    step_id: "resolve-right".to_owned(),
                    payload: slipbox_core::WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::NodeKey {
                            node_key: right_key.clone(),
                        },
                    },
                },
                slipbox_core::WorkflowStepSpec {
                    step_id: "compare".to_owned(),
                    payload: slipbox_core::WorkflowStepPayload::Compare {
                        left: slipbox_core::WorkflowStepRef {
                            step_id: "resolve-left".to_owned(),
                        },
                        right: slipbox_core::WorkflowStepRef {
                            step_id: "resolve-right".to_owned(),
                        },
                        group: NoteComparisonGroup::Overlap,
                        limit: 10,
                    },
                },
                slipbox_core::WorkflowStepSpec {
                    step_id: "save-artifact".to_owned(),
                    payload: slipbox_core::WorkflowStepPayload::ArtifactSave {
                        source: slipbox_core::WorkflowArtifactSaveSource::CompareStep {
                            step_id: "compare".to_owned(),
                        },
                        metadata: ExplorationArtifactMetadata {
                            artifact_id: artifact_id.to_owned(),
                            title: "Workflow Review Side Effect".to_owned(),
                            summary: None,
                        },
                        overwrite: true,
                    },
                },
            ],
        };
        fs::write(
            workflow_dir.join("side-effect.json"),
            serde_json::to_vec_pretty(&workflow).expect("workflow should serialize"),
        )
        .expect("workflow spec should be written");

        let existing_review_id = "review/workflow/side-effect";
        state
            .database
            .save_review_run(&sample_audit_review_run(
                existing_review_id,
                "Existing Review",
                ReviewFindingStatus::Open,
            ))
            .expect("existing review should be saved");
        assert!(
            state
                .database
                .exploration_artifact(artifact_id)
                .expect("artifact lookup should succeed")
                .is_none()
        );

        let conflict = save_workflow_review(
            &mut state,
            json!({
                "workflow_id": workflow_id,
                "review_id": existing_review_id,
                "overwrite": false
            }),
        )
        .expect_err("existing review should be rejected before workflow execution");
        assert_eq!(
            conflict.into_inner().message,
            format!("review run already exists: {existing_review_id}")
        );
        assert!(
            state
                .database
                .exploration_artifact(artifact_id)
                .expect("artifact lookup should still succeed")
                .is_none()
        );
    }

    #[test]
    fn execute_workflow_spec_runs_all_supported_step_kinds() {
        let (_workspace, mut state, left_key, right_key) = comparison_state();
        let workflow = WorkflowSpec {
            metadata: slipbox_core::WorkflowMetadata {
                workflow_id: "workflow/test-all-kinds".to_owned(),
                title: "All Kinds".to_owned(),
                summary: Some("Exercise all workflow step kinds".to_owned()),
            },
            compatibility: slipbox_core::WorkflowSpecCompatibility::default(),
            inputs: vec![
                slipbox_core::WorkflowInputSpec {
                    input_id: "left".to_owned(),
                    title: "Left".to_owned(),
                    summary: None,
                    kind: slipbox_core::WorkflowInputKind::NoteTarget,
                },
                slipbox_core::WorkflowInputSpec {
                    input_id: "right".to_owned(),
                    title: "Right".to_owned(),
                    summary: None,
                    kind: slipbox_core::WorkflowInputKind::NoteTarget,
                },
            ],
            steps: vec![
                slipbox_core::WorkflowStepSpec {
                    step_id: "resolve-left".to_owned(),
                    payload: slipbox_core::WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Input {
                            input_id: "left".to_owned(),
                        },
                    },
                },
                slipbox_core::WorkflowStepSpec {
                    step_id: "resolve-right".to_owned(),
                    payload: slipbox_core::WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Input {
                            input_id: "right".to_owned(),
                        },
                    },
                },
                slipbox_core::WorkflowStepSpec {
                    step_id: "compare".to_owned(),
                    payload: slipbox_core::WorkflowStepPayload::Compare {
                        left: slipbox_core::WorkflowStepRef {
                            step_id: "resolve-left".to_owned(),
                        },
                        right: slipbox_core::WorkflowStepRef {
                            step_id: "resolve-right".to_owned(),
                        },
                        group: NoteComparisonGroup::Tension,
                        limit: 10,
                    },
                },
                slipbox_core::WorkflowStepSpec {
                    step_id: "save".to_owned(),
                    payload: slipbox_core::WorkflowStepPayload::ArtifactSave {
                        source: slipbox_core::WorkflowArtifactSaveSource::CompareStep {
                            step_id: "compare".to_owned(),
                        },
                        metadata: ExplorationArtifactMetadata {
                            artifact_id: "workflow-saved-comparison".to_owned(),
                            title: "Workflow Saved Comparison".to_owned(),
                            summary: None,
                        },
                        overwrite: false,
                    },
                },
                slipbox_core::WorkflowStepSpec {
                    step_id: "run-saved".to_owned(),
                    payload: slipbox_core::WorkflowStepPayload::ArtifactRun {
                        artifact_id: "workflow-saved-comparison".to_owned(),
                    },
                },
            ],
        };

        let result = execute_workflow_spec(
            &mut state,
            &workflow,
            &[
                WorkflowInputAssignment {
                    input_id: "left".to_owned(),
                    target: WorkflowResolveTarget::NodeKey {
                        node_key: left_key.clone(),
                    },
                },
                WorkflowInputAssignment {
                    input_id: "right".to_owned(),
                    target: WorkflowResolveTarget::NodeKey {
                        node_key: right_key.clone(),
                    },
                },
            ],
        )
        .expect("workflow execution should succeed");

        assert_eq!(
            result.workflow.metadata.workflow_id,
            "workflow/test-all-kinds"
        );
        assert_eq!(
            result
                .steps
                .iter()
                .map(WorkflowStepReport::kind)
                .collect::<Vec<_>>(),
            vec![
                slipbox_core::WorkflowStepKind::Resolve,
                slipbox_core::WorkflowStepKind::Resolve,
                slipbox_core::WorkflowStepKind::Compare,
                slipbox_core::WorkflowStepKind::ArtifactSave,
                slipbox_core::WorkflowStepKind::ArtifactRun,
            ]
        );
        match &result.steps[4].payload {
            WorkflowStepReportPayload::ArtifactRun { artifact } => {
                assert_eq!(artifact.metadata.artifact_id, "workflow-saved-comparison");
                assert!(matches!(
                    artifact.payload,
                    ExecutedExplorationArtifactPayload::Comparison { .. }
                ));
            }
            other => panic!("expected artifact-run report, got {:?}", other.kind()),
        }
    }

    #[test]
    fn workflow_rpc_lists_shows_and_runs_built_ins() {
        let (_workspace, mut state, _target_key) = indexed_state();
        let anchor_key = state
            .database
            .anchor_at_point("alpha.org", 31)
            .expect("anchor lookup should succeed")
            .expect("anonymous heading anchor should exist")
            .node_key;

        let listed: ListWorkflowsResult = serde_json::from_value(
            list_workflows(&mut state, json!({})).expect("list workflows RPC should succeed"),
        )
        .expect("list workflows result should decode");
        assert_eq!(listed.workflows.len(), 5);
        assert_eq!(
            listed.workflows[0].metadata.workflow_id,
            BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID
        );
        assert_eq!(
            listed.workflows[2].metadata.workflow_id,
            BUILT_IN_WORKFLOW_PERIODIC_REVIEW_ID
        );
        assert_eq!(
            listed.workflows[3].metadata.workflow_id,
            BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID
        );
        assert_eq!(
            listed.workflows[4].metadata.workflow_id,
            BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID
        );

        let shown: WorkflowResult = serde_json::from_value(
            workflow(
                &mut state,
                json!({ "workflow_id": BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID }),
            )
            .expect("workflow RPC should succeed"),
        )
        .expect("workflow result should decode");
        assert_eq!(
            shown.workflow.metadata.workflow_id,
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
        );
        assert_eq!(shown.workflow.inputs.len(), 1);

        let executed_anchor_key = anchor_key.clone();
        let executed: RunWorkflowResult = serde_json::from_value(
            run_workflow(
                &mut state,
                json!({
                    "workflow_id": BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID,
                    "inputs": [
                        {
                            "input_id": "focus",
                            "kind": "node-key",
                            "node_key": anchor_key,
                        }
                    ]
                }),
            )
            .expect("run workflow RPC should succeed"),
        )
        .expect("run workflow result should decode");
        assert_eq!(
            executed.result.workflow.metadata.workflow_id,
            BUILT_IN_WORKFLOW_UNRESOLVED_SWEEP_ID
        );
        assert_eq!(executed.result.steps.len(), 4);
        assert_eq!(executed.result.steps[0].kind().label(), "resolve");
        match &executed.result.steps[1].payload {
            WorkflowStepReportPayload::Explore { result, .. } => {
                assert_eq!(result.lens, ExplorationLens::Unresolved);
            }
            other => panic!("expected unresolved explore report, got {:?}", other.kind()),
        }
        match &executed.result.steps[2].payload {
            WorkflowStepReportPayload::Explore {
                focus_node_key,
                result,
            } => {
                assert_eq!(focus_node_key, &executed_anchor_key);
                assert_eq!(result.lens, ExplorationLens::Tasks);
            }
            other => panic!("expected tasks explore report, got {:?}", other.kind()),
        }

        let weak: RunWorkflowResult = serde_json::from_value(
            run_workflow(
                &mut state,
                json!({
                    "workflow_id": BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID,
                    "inputs": [
                        {
                            "input_id": "focus",
                            "kind": "node-key",
                            "node_key": executed_anchor_key,
                        }
                    ]
                }),
            )
            .expect("weak integration review workflow should run"),
        )
        .expect("weak integration workflow result should decode");
        assert_eq!(
            weak.result.workflow.metadata.workflow_id,
            BUILT_IN_WORKFLOW_WEAK_INTEGRATION_REVIEW_ID
        );
        assert_eq!(weak.result.steps.len(), 4);
        match &weak.result.steps[1].payload {
            WorkflowStepReportPayload::Explore { result, .. } => {
                assert_eq!(result.lens, ExplorationLens::Unresolved);
            }
            other => panic!(
                "expected weak integration explore report, got {:?}",
                other.kind()
            ),
        }
    }

    #[test]
    fn workflow_rpc_reports_lookup_and_step_failures_with_context() {
        let (_workspace, mut state, target_key) = indexed_state();
        let anchor_key = state
            .database
            .anchor_at_point("alpha.org", 18)
            .expect("anchor lookup should succeed")
            .expect("anonymous heading anchor should exist")
            .node_key;

        let unknown_workflow = workflow(
            &mut state,
            json!({ "workflow_id": "workflow/builtin/missing" }),
        )
        .expect_err("unknown workflow should fail");
        assert_eq!(
            unknown_workflow.into_inner().message,
            "unknown workflow: workflow/builtin/missing"
        );

        let step_failure = run_workflow(
            &mut state,
            json!({
                "workflow_id": BUILT_IN_WORKFLOW_CONTEXT_SWEEP_ID,
                "inputs": [
                    {
                        "input_id": "focus",
                        "kind": "id",
                        "id": "missing-id"
                    }
                ]
            }),
        )
        .expect_err("missing workflow input target should fail");
        assert_eq!(
            step_failure.into_inner().message,
            "workflow step resolve-focus failed: unknown workflow focus target: missing-id"
        );

        let note_target_anchor_failure = run_workflow(
            &mut state,
            json!({
                "workflow_id": BUILT_IN_WORKFLOW_COMPARISON_TENSION_ID,
                "inputs": [
                    {
                        "input_id": "left",
                        "kind": "node-key",
                        "node_key": anchor_key,
                    },
                    {
                        "input_id": "right",
                        "kind": "node-key",
                        "node_key": target_key,
                    }
                ]
            }),
        )
        .expect_err("note-target workflow inputs should reject anchor node keys");
        assert_eq!(
            note_target_anchor_failure.into_inner().message,
            format!(
                "workflow step resolve-left failed: unknown workflow note target: {anchor_key}"
            )
        );
    }

    #[test]
    fn corpus_audit_rpc_dispatches_index_backed_audit_kinds() {
        let (_workspace, mut state) = audit_state();

        let dangling: CorpusAuditResult = serde_json::from_value(
            corpus_audit(
                &mut state,
                json!({ "audit": "dangling-links", "limit": 20 }),
            )
            .expect("dangling link audit should succeed"),
        )
        .expect("dangling audit result should decode");
        assert_eq!(dangling.audit, CorpusAuditKind::DanglingLinks);
        assert_eq!(dangling.entries.len(), 1);
        match &dangling.entries[0] {
            CorpusAuditEntry::DanglingLink { record } => {
                assert_eq!(record.source.title, "Dangling Source");
                assert_eq!(record.missing_explicit_id, "missing-id");
            }
            other => panic!("expected dangling link audit entry, got {:?}", other.kind()),
        }

        let duplicates: CorpusAuditResult = serde_json::from_value(
            corpus_audit(
                &mut state,
                json!({ "audit": "duplicate-titles", "limit": 20 }),
            )
            .expect("duplicate title audit should succeed"),
        )
        .expect("duplicate audit result should decode");
        assert_eq!(duplicates.audit, CorpusAuditKind::DuplicateTitles);
        assert_eq!(duplicates.entries.len(), 1);
        match &duplicates.entries[0] {
            CorpusAuditEntry::DuplicateTitle { record } => {
                assert_eq!(record.title, "Shared Title");
                assert_eq!(record.notes.len(), 2);
            }
            other => panic!(
                "expected duplicate title audit entry, got {:?}",
                other.kind()
            ),
        }

        let orphans: CorpusAuditResult = serde_json::from_value(
            corpus_audit(&mut state, json!({ "audit": "orphan-notes", "limit": 20 }))
                .expect("orphan note audit should succeed"),
        )
        .expect("orphan audit result should decode");
        assert_eq!(orphans.audit, CorpusAuditKind::OrphanNotes);
        assert_eq!(orphans.entries.len(), 1);
        match &orphans.entries[0] {
            CorpusAuditEntry::OrphanNote { record } => {
                assert_eq!(record.note.title, "Orphan");
                assert_eq!(record.reference_count, 0);
                assert_eq!(record.backlink_count, 0);
                assert_eq!(record.forward_link_count, 0);
            }
            other => panic!("expected orphan note audit entry, got {:?}", other.kind()),
        }

        let weak: CorpusAuditResult = serde_json::from_value(
            corpus_audit(
                &mut state,
                json!({ "audit": "weakly-integrated-notes", "limit": 20 }),
            )
            .expect("weakly integrated note audit should succeed"),
        )
        .expect("weak audit result should decode");
        assert_eq!(weak.audit, CorpusAuditKind::WeaklyIntegratedNotes);
        assert_eq!(weak.entries.len(), 1);
        match &weak.entries[0] {
            CorpusAuditEntry::WeaklyIntegratedNote { record } => {
                assert_eq!(record.note.title, "Weak");
                assert_eq!(record.reference_count, 1);
                assert_eq!(record.backlink_count, 0);
                assert_eq!(record.forward_link_count, 0);
            }
            other => panic!(
                "expected weakly integrated note audit entry, got {:?}",
                other.kind()
            ),
        }
    }

    fn saved_lens_artifact(
        artifact_id: &str,
        title: &str,
        node_key: &str,
        lens: ExplorationLens,
    ) -> SavedExplorationArtifact {
        SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: artifact_id.to_owned(),
                title: title.to_owned(),
                summary: None,
            },
            payload: ExplorationArtifactPayload::LensView {
                artifact: Box::new(SavedLensViewArtifact {
                    root_node_key: node_key.to_owned(),
                    current_node_key: node_key.to_owned(),
                    lens,
                    limit: 20,
                    unique: false,
                    frozen_context: false,
                }),
            },
        }
    }

    fn saved_comparison_artifact(
        artifact_id: &str,
        title: &str,
        left_node_key: &str,
        right_node_key: &str,
    ) -> SavedExplorationArtifact {
        SavedExplorationArtifact {
            metadata: ExplorationArtifactMetadata {
                artifact_id: artifact_id.to_owned(),
                title: title.to_owned(),
                summary: None,
            },
            payload: ExplorationArtifactPayload::Comparison {
                artifact: Box::new(SavedComparisonArtifact {
                    root_node_key: left_node_key.to_owned(),
                    left_node_key: left_node_key.to_owned(),
                    right_node_key: right_node_key.to_owned(),
                    active_lens: ExplorationLens::Structure,
                    structure_unique: false,
                    comparison_group: Default::default(),
                    limit: 20,
                    frozen_context: false,
                }),
            },
        }
    }

    fn sample_pack_workflow(workflow_id: &str, title: &str, node_key: &str) -> WorkflowSpec {
        WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: workflow_id.to_owned(),
                title: title.to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![WorkflowStepSpec {
                step_id: "explore-pack-focus".to_owned(),
                payload: WorkflowStepPayload::Explore {
                    focus: slipbox_core::WorkflowExploreFocus::NodeKey {
                        node_key: node_key.to_owned(),
                    },
                    lens: ExplorationLens::Refs,
                    limit: 20,
                    unique: false,
                },
            }],
        }
    }

    fn sample_input_workflow(workflow_id: &str, title: &str) -> WorkflowSpec {
        WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: workflow_id.to_owned(),
                title: title.to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: vec![slipbox_core::WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus".to_owned(),
                summary: None,
                kind: slipbox_core::WorkflowInputKind::NoteTarget,
            }],
            steps: vec![
                WorkflowStepSpec {
                    step_id: "resolve-focus".to_owned(),
                    payload: WorkflowStepPayload::Resolve {
                        target: WorkflowResolveTarget::Input {
                            input_id: "focus".to_owned(),
                        },
                    },
                },
                WorkflowStepSpec {
                    step_id: "explore-focus".to_owned(),
                    payload: WorkflowStepPayload::Explore {
                        focus: slipbox_core::WorkflowExploreFocus::ResolvedStep {
                            step_id: "resolve-focus".to_owned(),
                        },
                        lens: ExplorationLens::Refs,
                        limit: 20,
                        unique: false,
                    },
                },
            ],
        }
    }

    fn sample_artifact_save_workflow(workflow_id: &str, node_key: &str) -> WorkflowSpec {
        WorkflowSpec {
            metadata: WorkflowMetadata {
                workflow_id: workflow_id.to_owned(),
                title: "Artifact Save Workflow".to_owned(),
                summary: None,
            },
            compatibility: WorkflowSpecCompatibility::default(),
            inputs: Vec::new(),
            steps: vec![
                WorkflowStepSpec {
                    step_id: "explore-focus".to_owned(),
                    payload: WorkflowStepPayload::Explore {
                        focus: slipbox_core::WorkflowExploreFocus::NodeKey {
                            node_key: node_key.to_owned(),
                        },
                        lens: ExplorationLens::Refs,
                        limit: 20,
                        unique: false,
                    },
                },
                WorkflowStepSpec {
                    step_id: "save-artifact".to_owned(),
                    payload: WorkflowStepPayload::ArtifactSave {
                        source: slipbox_core::WorkflowArtifactSaveSource::ExploreStep {
                            step_id: "explore-focus".to_owned(),
                        },
                        metadata: ExplorationArtifactMetadata {
                            artifact_id: "routine-conflict-artifact".to_owned(),
                            title: "Routine Conflict Artifact".to_owned(),
                            summary: None,
                        },
                        overwrite: false,
                    },
                },
            ],
        }
    }

    fn sample_routine_report_profile(profile_id: &str) -> ReportProfileSpec {
        ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: profile_id.to_owned(),
                title: "Routine Detail".to_owned(),
                summary: None,
            },
            subjects: vec![
                ReportProfileSubject::Routine,
                ReportProfileSubject::Audit,
                ReportProfileSubject::Review,
                ReportProfileSubject::Diff,
            ],
            mode: ReportProfileMode::Detail,
            status_filters: Some(vec![ReviewFindingStatus::Open]),
            diff_buckets: Some(vec![ReviewRunDiffBucket::Unchanged]),
            jsonl_line_kinds: Some(vec![
                ReportJsonlLineKind::Routine,
                ReportJsonlLineKind::Audit,
                ReportJsonlLineKind::Entry,
                ReportJsonlLineKind::Review,
                ReportJsonlLineKind::Finding,
                ReportJsonlLineKind::Diff,
                ReportJsonlLineKind::Unchanged,
            ]),
        }
    }

    fn sample_routine_only_review_profile(profile_id: &str) -> ReportProfileSpec {
        ReportProfileSpec {
            metadata: ReportProfileMetadata {
                profile_id: profile_id.to_owned(),
                title: "Routine Review Lines".to_owned(),
                summary: None,
            },
            subjects: vec![ReportProfileSubject::Routine],
            mode: ReportProfileMode::Detail,
            status_filters: None,
            diff_buckets: None,
            jsonl_line_kinds: Some(vec![
                ReportJsonlLineKind::Review,
                ReportJsonlLineKind::Finding,
            ]),
        }
    }

    fn sample_audit_review_routine(routine_id: &str, profile_id: &str) -> ReviewRoutineSpec {
        ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: routine_id.to_owned(),
                title: "Duplicate Title Routine".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Audit {
                audit: CorpusAuditKind::DuplicateTitles,
                limit: 20,
            },
            inputs: Vec::new(),
            save_review: ReviewRoutineSaveReviewPolicy {
                enabled: true,
                review_id: Some("review/routine/001-current".to_owned()),
                title: Some("Routine Duplicate Title Review".to_owned()),
                summary: None,
                overwrite: false,
            },
            compare: Some(ReviewRoutineComparePolicy {
                target: ReviewRoutineCompareTarget::LatestCompatibleReview,
                report_profile_id: Some(profile_id.to_owned()),
            }),
            report_profile_ids: vec![profile_id.to_owned()],
        }
    }

    fn sample_workflow_review_routine(
        routine_id: &str,
        workflow_id: &str,
        review_id: Option<&str>,
    ) -> ReviewRoutineSpec {
        ReviewRoutineSpec {
            metadata: ReviewRoutineMetadata {
                routine_id: routine_id.to_owned(),
                title: "Workflow Routine".to_owned(),
                summary: None,
            },
            source: ReviewRoutineSource::Workflow {
                workflow_id: workflow_id.to_owned(),
            },
            inputs: vec![slipbox_core::WorkflowInputSpec {
                input_id: "focus".to_owned(),
                title: "Focus".to_owned(),
                summary: None,
                kind: slipbox_core::WorkflowInputKind::NoteTarget,
            }],
            save_review: ReviewRoutineSaveReviewPolicy {
                enabled: review_id.is_some(),
                review_id: review_id.map(str::to_owned),
                title: None,
                summary: None,
                overwrite: false,
            },
            compare: None,
            report_profile_ids: Vec::new(),
        }
    }

    fn sample_workbench_pack(pack_id: &str, title: &str) -> WorkbenchPackManifest {
        WorkbenchPackManifest {
            metadata: WorkbenchPackMetadata {
                pack_id: pack_id.to_owned(),
                title: title.to_owned(),
                summary: Some("Reusable workbench assets".to_owned()),
            },
            compatibility: WorkbenchPackCompatibility::default(),
            workflows: Vec::new(),
            review_routines: Vec::new(),
            report_profiles: vec![ReportProfileSpec {
                metadata: ReportProfileMetadata {
                    profile_id: format!("{pack_id}/profile/detail"),
                    title: "Detail Report".to_owned(),
                    summary: None,
                },
                subjects: vec![ReportProfileSubject::Audit],
                mode: ReportProfileMode::Detail,
                status_filters: None,
                diff_buckets: None,
                jsonl_line_kinds: None,
            }],
            entrypoint_routine_ids: Vec::new(),
        }
    }

    fn sample_audit_review_run(
        review_id: &str,
        title: &str,
        status: ReviewFindingStatus,
    ) -> ReviewRun {
        ReviewRun {
            metadata: ReviewRunMetadata {
                review_id: review_id.to_owned(),
                title: title.to_owned(),
                summary: Some("Review dangling links".to_owned()),
            },
            payload: ReviewRunPayload::Audit {
                audit: CorpusAuditKind::DanglingLinks,
                limit: 200,
            },
            findings: vec![ReviewFinding {
                finding_id: "audit/dangling-links/source/missing-id".to_owned(),
                status,
                payload: ReviewFindingPayload::Audit {
                    entry: Box::new(CorpusAuditEntry::DanglingLink {
                        record: Box::new(DanglingLinkAuditRecord {
                            source: AnchorRecord {
                                node_key: "file:source.org".to_owned(),
                                explicit_id: Some("source-id".to_owned()),
                                file_path: "source.org".to_owned(),
                                title: "Source".to_owned(),
                                outline_path: "Source".to_owned(),
                                aliases: Vec::new(),
                                tags: Vec::new(),
                                refs: Vec::new(),
                                todo_keyword: None,
                                scheduled_for: None,
                                deadline_for: None,
                                closed_at: None,
                                level: 0,
                                line: 1,
                                kind: NodeKind::File,
                                file_mtime_ns: 0,
                                backlink_count: 0,
                                forward_link_count: 0,
                            },
                            missing_explicit_id: "missing-id".to_owned(),
                            line: 12,
                            column: 7,
                            preview: "[[id:missing-id][Missing]]".to_owned(),
                        }),
                    }),
                },
            }],
        }
    }

    fn indexed_state() -> (TempDir, ServerState, String) {
        let workspace = tempfile::tempdir().expect("workspace should be created");
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).expect("notes root should be created");
        fs::write(
            root.join("alpha.org"),
            r#"#+title: Alpha

* Source
:PROPERTIES:
:ID: source-id
:END:
Points to [[id:target-id]].

* TODO Target
:PROPERTIES:
:ID: target-id
:ROAM_REFS: cite:smith2024
:END:
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Target body.

* Reflink Source
This mentions cite:smith2024 near Target.

* TODO Dual Match Peer
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Shares both planning dates and task state.

* NEXT Cross Match Peer
SCHEDULED: <2026-05-03 Sat>
DEADLINE: <2026-05-01 Thu>
Shares both planning dates through opposite fields.

* TODO Keyword Only Peer
Shares only the same task state.

* WAIT Deadline Peer
DEADLINE: <2026-05-03 Sat>
Shares only the target deadline.
"#,
        )
        .expect("fixture should be written");

        let db_path = workspace.path().join("index.sqlite3");
        let discovery = DiscoveryPolicy::default();
        let mut state = ServerState::new(root.clone(), db_path, Vec::new(), discovery)
            .expect("state should be created");
        let files =
            scan_root_with_policy(&root, &state.discovery).expect("fixture should be indexed");
        state
            .database
            .sync_index(&files)
            .expect("fixture index should sync");
        let target_key = state
            .database
            .node_from_id("target-id")
            .expect("target note lookup should succeed")
            .expect("target note should exist")
            .node_key;

        (workspace, state, target_key)
    }

    fn comparison_state() -> (TempDir, ServerState, String, String) {
        let workspace = tempfile::tempdir().expect("workspace should be created");
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).expect("notes root should be created");
        fs::write(
            root.join("comparison.org"),
            r#"#+title: Comparison

* TODO Left
:PROPERTIES:
:ID: left-id
:ROAM_REFS: cite:shared2024 cite:left2024
:END:
SCHEDULED: <2026-05-01 Thu>
Links to [[id:shared-forward-id]] and [[id:left-right-bridge-id]].

* NEXT Right
:PROPERTIES:
:ID: right-id
:ROAM_REFS: cite:shared2024 cite:right2024
:END:
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-01 Thu>
Links to [[id:shared-forward-id]] and [[id:right-left-bridge-id]].

* Shared Forward
:PROPERTIES:
:ID: shared-forward-id
:END:
Forward target body.

* Left To Right Bridge
:PROPERTIES:
:ID: left-right-bridge-id
:END:
Connects to [[id:right-id]].

* Right To Left Bridge
:PROPERTIES:
:ID: right-left-bridge-id
:END:
Connects to [[id:left-id]].

* Shared Backlink
:PROPERTIES:
:ID: shared-backlink-id
:END:
Links to [[id:left-id]] and [[id:right-id]].
"#,
        )
        .expect("fixture should be written");

        let db_path = workspace.path().join("index.sqlite3");
        let discovery = DiscoveryPolicy::default();
        let mut state = ServerState::new(root.clone(), db_path, Vec::new(), discovery)
            .expect("state should be created");
        let files =
            scan_root_with_policy(&root, &state.discovery).expect("fixture should be indexed");
        state
            .database
            .sync_index(&files)
            .expect("fixture index should sync");
        let left_key = state
            .database
            .node_from_id("left-id")
            .expect("left note lookup should succeed")
            .expect("left note should exist")
            .node_key;
        let right_key = state
            .database
            .node_from_id("right-id")
            .expect("right note lookup should succeed")
            .expect("right note should exist")
            .node_key;

        (workspace, state, left_key, right_key)
    }

    fn audit_state() -> (TempDir, ServerState) {
        let workspace = tempfile::tempdir().expect("workspace should be created");
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).expect("notes root should be created");
        fs::write(
            root.join("duplicate-a.org"),
            r#":PROPERTIES:
:ID: dup-a-id
:END:
#+title: Shared Title

Links to [[id:dup-b-id][Other duplicate]].
"#,
        )
        .expect("fixture should be written");
        fs::write(
            root.join("duplicate-b.org"),
            r#":PROPERTIES:
:ID: dup-b-id
:END:
#+title: shared title

Links to [[id:dup-a-id][Other duplicate]].
"#,
        )
        .expect("fixture should be written");
        fs::write(
            root.join("dangling-source.org"),
            r#":PROPERTIES:
:ID: dangling-source-id
:END:
#+title: Dangling Source

Points to [[id:missing-id][Missing]].
"#,
        )
        .expect("fixture should be written");
        fs::write(
            root.join("orphan.org"),
            r#":PROPERTIES:
:ID: orphan-id
:END:
#+title: Orphan

Just an orphan note.
"#,
        )
        .expect("fixture should be written");
        fs::write(
            root.join("weak.org"),
            r#":PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:weak2024
:END:
#+title: Weak

Has refs but no structural links.
"#,
        )
        .expect("fixture should be written");

        let db_path = workspace.path().join("index.sqlite3");
        let discovery = DiscoveryPolicy::default();
        let mut state = ServerState::new(root.clone(), db_path, Vec::new(), discovery)
            .expect("state should be created");
        let files =
            scan_root_with_policy(&root, &state.discovery).expect("fixture should be indexed");
        state
            .database
            .sync_index(&files)
            .expect("fixture index should sync");

        (workspace, state)
    }

    fn non_obvious_state() -> (TempDir, ServerState, String) {
        let workspace = tempfile::tempdir().expect("workspace should be created");
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).expect("notes root should be created");
        fs::write(
            root.join("older.org"),
            r#"#+title: Older

* Dormant Bridge
:PROPERTIES:
:ID: dormant-bridge-id
:ROAM_REFS: cite:shared2024
:END:
Links to [[id:neighbor-id]] and [[id:support-id]].

* Support
:PROPERTIES:
:ID: support-id
:END:
Support body.
"#,
        )
        .expect("older fixture should be written");
        sleep(Duration::from_millis(10));
        fs::write(
            root.join("focus.org"),
            r#"#+title: Focus

* Focus
:PROPERTIES:
:ID: focus-id
:ROAM_REFS: cite:shared2024 cite:focus2024
:END:
Links to [[id:neighbor-id]].

* Neighbor
:PROPERTIES:
:ID: neighbor-id
:END:
Neighbor body.
"#,
        )
        .expect("focus fixture should be written");
        sleep(Duration::from_millis(10));
        fs::write(
            root.join("related.org"),
            r#"#+title: Related

* TODO Unresolved Thread
:PROPERTIES:
:ID: unresolved-id
:ROAM_REFS: cite:shared2024
:END:
Unresolved body.

* Weak Thread
:PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:shared2024
:END:
Weakly integrated body.
"#,
        )
        .expect("related fixture should be written");

        let db_path = workspace.path().join("index.sqlite3");
        let discovery = DiscoveryPolicy::default();
        let mut state = ServerState::new(root.clone(), db_path, Vec::new(), discovery)
            .expect("state should be created");
        let files =
            scan_root_with_policy(&root, &state.discovery).expect("fixture should be indexed");
        state
            .database
            .sync_index(&files)
            .expect("fixture index should sync");
        let focus_key = state
            .database
            .node_from_id("focus-id")
            .expect("focus note lookup should succeed")
            .expect("focus note should exist")
            .node_key;

        (workspace, state, focus_key)
    }
}
