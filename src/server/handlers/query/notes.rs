use slipbox_core::{
    NodeAtPointParams, NodeFromIdParams, NodeFromKeyParams, NodeFromTitleOrAliasParams,
    RandomNodeResult, SearchNodesParams, SearchNodesResult,
};
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;

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
