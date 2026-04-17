use slipbox_core::{
    AgendaParams, AgendaResult, BacklinksParams, BacklinksResult, ForwardLinksParams,
    ForwardLinksResult, GraphParams, GraphResult, IndexFileParams, IndexedFilesResult,
    NodeAtPointParams, NodeFromIdParams, NodeFromRefParams, NodeFromTitleOrAliasParams, PingInfo,
    RandomNodeResult, ReflinksParams, ReflinksResult, SearchFilesParams, SearchFilesResult,
    SearchNodesParams, SearchNodesResult, SearchOccurrencesParams, SearchOccurrencesResult,
    SearchRefsParams, SearchRefsResult, SearchTagsParams, SearchTagsResult, StatusInfo,
    UnlinkedReferencesParams, UnlinkedReferencesResult,
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
