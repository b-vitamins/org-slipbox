use slipbox_core::{
    AgendaParams, AgendaResult, BacklinksParams, BacklinksResult, CompareNotesParams,
    DeleteExplorationArtifactResult, ExecuteExplorationArtifactResult, ExecutedExplorationArtifact,
    ExecutedExplorationArtifactPayload, ExplorationArtifactIdParams, ExplorationArtifactResult,
    ExplorationArtifactSummary, ExplorationEntry, ExplorationLens, ExplorationSection,
    ExplorationSectionKind, ExploreParams, ExploreResult, ForwardLinksParams, ForwardLinksResult,
    GraphParams, GraphResult, IndexFileParams, IndexedFilesResult, ListExplorationArtifactsParams,
    ListExplorationArtifactsResult, NodeAtPointParams, NodeFromIdParams, NodeFromKeyParams,
    NodeFromRefParams, NodeFromTitleOrAliasParams, NodeRecord, NoteComparisonResult, PingInfo,
    RandomNodeResult, ReflinksParams, ReflinksResult, SaveExplorationArtifactParams,
    SaveExplorationArtifactResult, SavedComparisonArtifact, SavedExplorationArtifact,
    SavedLensViewArtifact, SavedTrailStep, SearchFilesParams, SearchFilesResult, SearchNodesParams,
    SearchNodesResult, SearchOccurrencesParams, SearchOccurrencesResult, SearchRefsParams,
    SearchRefsResult, SearchTagsParams, SearchTagsResult, StatusInfo, TrailReplayResult,
    TrailReplayStepResult, UnlinkedReferencesParams, UnlinkedReferencesResult,
};
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

use crate::occurrences_query::query_occurrences;
use crate::reflinks_query::query_reflinks;
use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;
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
    let root_note = state.known_note(&artifact.root_node_key, "saved lens-view root note")?;
    let current_note =
        state.known_note(&artifact.current_node_key, "saved lens-view current note")?;
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

fn validate_artifact_id_params(params: &ExplorationArtifactIdParams) -> Result<(), JsonRpcError> {
    if let Some(message) = params.validation_error() {
        return Err(invalid_request(message));
    }
    Ok(())
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
    state
        .database
        .save_exploration_artifact(&params.artifact)
        .map_err(|error| internal_error(error.context("failed to save exploration artifact")))?;
    to_value(SaveExplorationArtifactResult {
        artifact: ExplorationArtifactSummary::from(&params.artifact),
    })
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
    to_value(serde_json::json!({ "file_path": relative_path }))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    use serde_json::json;
    use slipbox_core::{
        CompareNotesParams, ComparisonConnectorDirection, DeleteExplorationArtifactResult,
        ExecuteExplorationArtifactResult, ExecutedExplorationArtifactPayload,
        ExplorationArtifactMetadata, ExplorationArtifactPayload, ExplorationArtifactResult,
        ExplorationEntry, ExplorationExplanation, ExplorationLens, ExplorationSectionKind,
        ExploreParams, ExploreResult, ListExplorationArtifactsResult, NoteComparisonEntry,
        NoteComparisonExplanation, NoteComparisonResult, NoteComparisonSectionKind,
        SaveExplorationArtifactResult, SavedComparisonArtifact, SavedExplorationArtifact,
        SavedLensViewArtifact, SavedTrailArtifact, SavedTrailStep, TrailReplayStepResult,
    };
    use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};
    use tempfile::TempDir;

    use super::{
        compare_notes, delete_exploration_artifact, execute_compare_notes_query,
        execute_exploration_artifact, execute_explore_query, execute_saved_exploration_artifact,
        execute_saved_exploration_artifact_by_id, exploration_artifact, explore,
        list_exploration_artifacts, save_exploration_artifact,
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
            save_exploration_artifact(&mut state, json!({ "artifact": artifact.clone() }))
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
                save_exploration_artifact(&mut state, json!({ "artifact": artifact }))
                    .expect("save artifact RPC should succeed"),
            )
            .expect("save result should decode");
        }

        let root = state.root.clone();
        let db_path = state.db_path.clone();
        let discovery = state.discovery.clone();
        drop(state);

        let mut reopened =
            ServerState::new(root, db_path, discovery).expect("state should reopen cleanly");

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
        let mut state =
            ServerState::new(root.clone(), db_path, discovery).expect("state should be created");
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
        let mut state =
            ServerState::new(root.clone(), db_path, discovery).expect("state should be created");
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
        let mut state =
            ServerState::new(root.clone(), db_path, discovery).expect("state should be created");
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
