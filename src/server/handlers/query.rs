use slipbox_core::{
    AgendaParams, AgendaResult, BacklinksParams, BacklinksResult, CompareNotesParams,
    ExplorationEntry, ExplorationLens, ExplorationSection, ExplorationSectionKind, ExploreParams,
    ExploreResult, ForwardLinksParams, ForwardLinksResult, GraphParams, GraphResult,
    IndexFileParams, IndexedFilesResult, NodeAtPointParams, NodeFromIdParams, NodeFromRefParams,
    NodeFromTitleOrAliasParams, PingInfo, RandomNodeResult, ReflinksParams, ReflinksResult,
    SearchFilesParams, SearchFilesResult, SearchNodesParams, SearchNodesResult,
    SearchOccurrencesParams, SearchOccurrencesResult, SearchRefsParams, SearchRefsResult,
    SearchTagsParams, SearchTagsResult, StatusInfo, UnlinkedReferencesParams,
    UnlinkedReferencesResult,
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

pub(crate) fn explore(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ExploreParams = parse_params(params)?;
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
    };

    to_value(ExploreResult {
        lens: params.lens,
        sections,
    })
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
    let left = state.known_note(&params.left_node_key, "left comparison note")?;
    let right = state.known_note(&params.right_node_key, "right comparison note")?;
    let comparison = state
        .database
        .compare_notes(&left, &right, &params)
        .map_err(|error| internal_error(error.context("failed to compare notes")))?;
    to_value(comparison)
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

    use serde_json::json;
    use slipbox_core::{
        ComparisonConnectorDirection, ExplorationEntry, ExplorationSectionKind, ExploreResult,
        NoteComparisonEntry, NoteComparisonExplanation, NoteComparisonResult,
        NoteComparisonSectionKind,
    };
    use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};
    use tempfile::TempDir;

    use super::{compare_notes, explore};
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
                NoteComparisonSectionKind::LeftOnlyRefs,
                NoteComparisonSectionKind::RightOnlyRefs,
                NoteComparisonSectionKind::SharedBacklinks,
                NoteComparisonSectionKind::SharedForwardLinks,
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
            NoteComparisonEntry::Reference { record }
            if record.reference == "@left2024"
                && record.explanation == NoteComparisonExplanation::LeftOnlyReference
        )));
        assert!(comparison.sections[2].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Reference { record }
            if record.reference == "@right2024"
                && record.explanation == NoteComparisonExplanation::RightOnlyReference
        )));
        assert!(comparison.sections[3].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Node { record }
            if record.node.title == "Shared Backlink"
                && record.explanation == NoteComparisonExplanation::SharedBacklink
        )));
        assert!(comparison.sections[4].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Node { record }
            if record.node.title == "Shared Forward"
                && record.explanation == NoteComparisonExplanation::SharedForwardLink
        )));
        assert!(comparison.sections[5].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Node { record }
            if record.node.title == "Left To Right Bridge"
                && record.explanation == NoteComparisonExplanation::IndirectConnector {
                    direction: ComparisonConnectorDirection::LeftToRight,
                }
        )));
        assert!(comparison.sections[5].entries.iter().any(|entry| matches!(
            entry,
            NoteComparisonEntry::Node { record }
            if record.node.title == "Right To Left Bridge"
                && record.explanation == NoteComparisonExplanation::IndirectConnector {
                    direction: ComparisonConnectorDirection::RightToLeft,
                }
        )));
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
Target body.

* Reflink Source
This mentions cite:smith2024 near Target.

* TODO Peer Task
SCHEDULED: <2026-05-01 Thu>
Peer task body.
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

* Left
:PROPERTIES:
:ID: left-id
:ROAM_REFS: cite:shared2024 cite:left2024
:END:
Links to [[id:shared-forward-id]] and [[id:left-right-bridge-id]].

* Right
:PROPERTIES:
:ID: right-id
:ROAM_REFS: cite:shared2024 cite:right2024
:END:
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
}
