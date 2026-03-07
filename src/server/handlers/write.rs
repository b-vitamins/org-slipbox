use anyhow::anyhow;
use slipbox_core::{
    AppendHeadingAtOutlinePathParams, AppendHeadingParams, AppendHeadingToNodeParams,
    CaptureNodeParams, CaptureTemplateParams, EnsureFileNodeParams, EnsureNodeIdParams,
    ExtractSubtreeParams, RefileSubtreeParams, RewriteFileParams, UpdateNodeMetadataParams,
};
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::{
    ServerState, read_known_node, read_required_node, read_required_node_by_id,
    remove_deleted_paths, resolve_index_path, sync_changed_paths, sync_one_path,
};

pub(crate) fn capture_node(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: CaptureNodeParams = parse_params(params)?;
    let refs = params.normalized_refs();
    let captured = match params.file_path.as_deref() {
        Some(file_path) => {
            if let Some(head) = params.head.as_deref() {
                slipbox_write::capture_file_note_at_with_head_and_refs(
                    &state.root,
                    file_path,
                    &params.title,
                    head,
                    &refs,
                )
            } else {
                slipbox_write::capture_file_note_at_with_refs(
                    &state.root,
                    file_path,
                    &params.title,
                    &refs,
                )
            }
        }
        None => slipbox_write::capture_file_note_with_refs(&state.root, &params.title, &refs),
    }
    .map_err(|error| internal_error(error.context("failed to capture node")))?;
    sync_one_path(state, &captured.absolute_path)?;
    let node = read_required_node(state, &captured.node_key, "captured node")?;
    to_value(node)
}

pub(crate) fn capture_template(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let mut params: CaptureTemplateParams = parse_params(params)?;
    if let Some(file_path) = params.file_path.as_deref() {
        let (relative_path, _) = resolve_index_path(&state.root, file_path)
            .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
        params.file_path = Some(relative_path);
    }
    let target = match params.node_key.as_deref() {
        Some(node_key) => Some(read_known_node(state, node_key, "target node")?),
        None => None,
    };
    let captured = slipbox_write::capture_template(&state.root, target.as_ref(), &params)
        .map_err(|error| internal_error(error.context("failed to capture template")))?;
    sync_one_path(state, &captured.absolute_path)?;
    let node = read_required_node(state, &captured.node_key, "captured template")?;
    to_value(node)
}

pub(crate) fn ensure_file_node(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: EnsureFileNodeParams = parse_params(params)?;
    let ensured = slipbox_write::ensure_file_note(&state.root, &params.file_path, &params.title)
        .map_err(|error| internal_error(error.context("failed to ensure file node")))?;
    sync_one_path(state, &ensured.absolute_path)?;
    let node = read_required_node(state, &ensured.node_key, "ensured file node")?;
    to_value(node)
}

pub(crate) fn append_heading(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AppendHeadingParams = parse_params(params)?;
    let captured = slipbox_write::append_heading(
        &state.root,
        &params.file_path,
        &params.title,
        &params.heading,
        params.normalized_level(),
    )
    .map_err(|error| internal_error(error.context("failed to append heading")))?;
    sync_one_path(state, &captured.absolute_path)?;
    let node = read_required_node(state, &captured.node_key, "captured heading")?;
    to_value(node)
}

pub(crate) fn append_heading_to_node(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AppendHeadingToNodeParams = parse_params(params)?;
    let target = state
        .database
        .node_by_key(&params.node_key)
        .map_err(|error| internal_error(error.context("failed to fetch target node")))?
        .ok_or_else(|| {
            JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
                "unknown node: {}",
                params.node_key
            )))
        })?;
    let captured = slipbox_write::append_heading_to_node(&state.root, &target, &params.heading)
        .map_err(|error| internal_error(error.context("failed to append heading to node")))?;
    sync_one_path(state, &captured.absolute_path)?;
    let node = read_required_node(state, &captured.node_key, "captured heading")?;
    to_value(node)
}

pub(crate) fn append_heading_at_outline_path(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AppendHeadingAtOutlinePathParams = parse_params(params)?;
    let captured = slipbox_write::append_heading_at_outline_path(
        &state.root,
        &params.file_path,
        &params.heading,
        &params.normalized_outline_path(),
        params.head.as_deref(),
    )
    .map_err(|error| internal_error(error.context("failed to append heading at outline path")))?;
    sync_one_path(state, &captured.absolute_path)?;
    let node = read_required_node(state, &captured.node_key, "captured heading")?;
    to_value(node)
}

pub(crate) fn ensure_node_id(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: EnsureNodeIdParams = parse_params(params)?;
    let node = state
        .database
        .node_by_key(&params.node_key)
        .map_err(|error| internal_error(error.context("failed to fetch node")))?
        .ok_or_else(|| {
            JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
                "unknown node: {}",
                params.node_key
            )))
        })?;

    if node.explicit_id.is_none() {
        let updated_path = slipbox_write::ensure_node_id(&state.root, &node)
            .map_err(|error| internal_error(error.context("failed to assign node ID")))?;
        sync_one_path(state, &updated_path)?;
    }

    let refreshed = state
        .database
        .node_by_key(&params.node_key)
        .map_err(|error| internal_error(error.context("failed to read refreshed node")))?
        .ok_or_else(|| {
            internal_error(anyhow!(
                "node {} disappeared after ID update",
                params.node_key
            ))
        })?;
    to_value(refreshed)
}

pub(crate) fn update_node_metadata(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: UpdateNodeMetadataParams = parse_params(params)?;
    let node = read_known_node(state, &params.node_key, "node")?;
    let updated_path = slipbox_write::update_node_metadata(
        &state.root,
        &node,
        &slipbox_write::MetadataUpdate {
            aliases: params.normalized_aliases(),
            refs: params.normalized_refs(),
            tags: params.normalized_tags(),
        },
    )
    .map_err(|error| internal_error(error.context("failed to update node metadata")))?;
    sync_one_path(state, &updated_path)?;
    let refreshed = read_required_node(state, &params.node_key, "updated node")?;
    to_value(refreshed)
}

pub(crate) fn refile_subtree(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RefileSubtreeParams = parse_params(params)?;
    let source = read_known_node(state, &params.source_node_key, "source node")?;
    let target = read_known_node(state, &params.target_node_key, "target node")?;
    let outcome = slipbox_write::refile_subtree(&state.root, &source, &target)
        .map_err(|error| internal_error(error.context("failed to refile subtree")))?;
    sync_changed_paths(state, &outcome.changed_paths)?;
    remove_deleted_paths(state, &outcome.removed_paths)?;
    let moved = read_required_node_by_id(state, &outcome.explicit_id, "refiled node")?;
    to_value(moved)
}

pub(crate) fn extract_subtree(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ExtractSubtreeParams = parse_params(params)?;
    let source = read_known_node(state, &params.source_node_key, "source node")?;
    let (relative_path, _) = resolve_index_path(&state.root, &params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let outcome = slipbox_write::extract_subtree(&state.root, &source, &relative_path)
        .map_err(|error| internal_error(error.context("failed to extract subtree")))?;
    sync_changed_paths(state, &outcome.changed_paths)?;
    let extracted = read_required_node_by_id(state, &outcome.explicit_id, "extracted node")?;
    to_value(extracted)
}

pub(crate) fn promote_entire_file(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RewriteFileParams = parse_params(params)?;
    let (relative_path, absolute_path) = resolve_index_path(&state.root, &params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let outcome = slipbox_write::promote_entire_file(&state.root, &relative_path)
        .map_err(|error| internal_error(error.context("failed to promote file node")))?;
    sync_one_path(state, &absolute_path)?;
    let refreshed = read_required_node(state, &outcome.node_key, "promoted file node")?;
    to_value(refreshed)
}

pub(crate) fn demote_entire_file(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RewriteFileParams = parse_params(params)?;
    let (relative_path, absolute_path) = resolve_index_path(&state.root, &params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let outcome = slipbox_write::demote_entire_file(&state.root, &relative_path)
        .map_err(|error| internal_error(error.context("failed to demote file node")))?;
    sync_one_path(state, &absolute_path)?;
    let refreshed = read_required_node(state, &outcome.node_key, "demoted file node")?;
    to_value(refreshed)
}
