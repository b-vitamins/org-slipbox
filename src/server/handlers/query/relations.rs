use slipbox_core::{
    AgendaParams, AgendaResult, BacklinksParams, BacklinksResult, ForwardLinksParams,
    ForwardLinksResult, GraphParams, GraphResult, NodeFromRefParams, ReflinksParams,
    ReflinksResult, SearchOccurrencesParams, SearchOccurrencesResult, SearchRefsParams,
    SearchRefsResult, SearchTagsParams, SearchTagsResult, UnlinkedReferencesParams,
    UnlinkedReferencesResult,
};
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

use crate::occurrences_query::query_occurrences;
use crate::reflinks_query::query_reflinks;
use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;
use crate::unlinked_references_query::query_unlinked_references;

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
